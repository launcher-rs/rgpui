use anyhow::{Context as _, Result};
use etagere::{BucketedAtlasAllocator, size2};
use parking_lot::Mutex;
use rgpui::collections::FxHashMap;
use rgpui::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTextureList, AtlasTile, Bounds, DevicePixels,
    PlatformAtlas, Point, Size,
};
use std::{borrow::Cow, ops, sync::Arc};

use crate::WgpuContext;

fn device_size_to_etagere(size: Size<DevicePixels>) -> etagere::Size {
    size2(size.width.0, size.height.0)
}

fn etagere_point_to_device(point: etagere::Point) -> Point<DevicePixels> {
    Point {
        x: DevicePixels(point.x),
        y: DevicePixels(point.y),
    }
}

pub struct WgpuAtlas(Mutex<WgpuAtlasState>);

struct PendingUpload {
    id: AtlasTextureId,
    bounds: Bounds<DevicePixels>,
    data: Vec<u8>,
}

struct WgpuAtlasState {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    max_texture_size: u32,
    color_texture_format: wgpu::TextureFormat,
    storage: WgpuAtlasStorage,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
    pending_uploads: Vec<PendingUpload>,
}

pub struct WgpuTextureInfo {
    pub view: wgpu::TextureView,
}

impl WgpuAtlas {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        color_texture_format: wgpu::TextureFormat,
    ) -> Self {
        let max_texture_size = device.limits().max_texture_dimension_2d;
        WgpuAtlas(Mutex::new(WgpuAtlasState {
            device,
            queue,
            max_texture_size,
            color_texture_format,
            storage: WgpuAtlasStorage::default(),
            tiles_by_key: Default::default(),
            pending_uploads: Vec::new(),
        }))
    }

    pub fn from_context(context: &WgpuContext) -> Self {
        Self::new(
            context.device.clone(),
            context.queue.clone(),
            context.color_texture_format(),
        )
    }

    pub fn before_frame(&self) {
        let mut lock = self.0.lock();
        lock.flush_uploads();
    }

    pub fn get_texture_info(&self, id: AtlasTextureId) -> WgpuTextureInfo {
        let lock = self.0.lock();
        let texture = &lock.storage[id];
        WgpuTextureInfo {
            view: texture.view.clone(),
        }
    }

    /// Clears all cached textures and tiles, forcing them to be recreated.
    /// Use this for incremental recovery when the device is still valid.
    pub fn clear(&self) {
        let mut lock = self.0.lock();
        lock.storage = WgpuAtlasStorage::default();
        lock.tiles_by_key.clear();
        lock.pending_uploads.clear();
    }

    /// Handles device lost by clearing all textures and cached tiles.
    /// The atlas will lazily recreate textures as needed on subsequent frames.
    pub fn handle_device_lost(&self, context: &WgpuContext) {
        let mut lock = self.0.lock();
        lock.device = context.device.clone();
        lock.queue = context.queue.clone();
        lock.color_texture_format = context.color_texture_format();
        lock.storage = WgpuAtlasStorage::default();
        lock.tiles_by_key.clear();
        lock.pending_uploads.clear();
    }
}

impl PlatformAtlas for WgpuAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(Some(*tile))
        } else {
            profiling::scope!("new tile");
            let Some((size, bytes)) = build()? else {
                return Ok(None);
            };
            let tile = lock
                .allocate(size, key.texture_kind())
                .context("failed to allocate")?;
            lock.upload_texture(tile.texture_id, tile.bounds, &bytes);
            lock.tiles_by_key.insert(key.clone(), tile);
            Ok(Some(tile))
        }
    }

    fn remove(&self, key: &AtlasKey) {
        let mut lock = self.0.lock();

        let Some(tile) = lock.tiles_by_key.remove(key) else {
            return;
        };
        let id = tile.texture_id;

        let Some(texture_slot) = lock.storage[id.kind].textures.get_mut(id.index as usize) else {
            return;
        };

        if let Some(mut texture) = texture_slot.take() {
            texture.allocator.deallocate(tile.tile_id.into());
            texture.decrement_ref_count();
            if texture.is_unreferenced() {
                lock.pending_uploads
                    .retain(|upload| upload.id != texture.id);
                lock.storage[id.kind]
                    .free_list
                    .push(texture.id.index as usize);
            } else {
                *texture_slot = Some(texture);
            }
        }
    }
}

impl WgpuAtlasState {
    fn allocate(
        &mut self,
        size: Size<DevicePixels>,
        texture_kind: AtlasTextureKind,
    ) -> Option<AtlasTile> {
        {
            let textures = &mut self.storage[texture_kind];

            if let Some(tile) = textures
                .iter_mut()
                .rev()
                .find_map(|texture| texture.allocate(size))
            {
                return Some(tile);
            }
        }

        let texture = self.push_texture(size, texture_kind);
        texture.allocate(size)
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> &mut WgpuAtlasTexture {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        let max_texture_size = self.max_texture_size as i32;
        let max_atlas_size = Size {
            width: DevicePixels(max_texture_size),
            height: DevicePixels(max_texture_size),
        };

        let size = min_size.min(&max_atlas_size).max(&DEFAULT_ATLAS_SIZE);
        let format = match kind {
            AtlasTextureKind::Monochrome => wgpu::TextureFormat::R8Unorm,
            AtlasTextureKind::Subpixel | AtlasTextureKind::Polychrome => self.color_texture_format,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d {
                width: size.width.0 as u32,
                height: size.height.0 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let texture_list = &mut self.storage[kind];
        let index = texture_list.free_list.pop();

        let atlas_texture = WgpuAtlasTexture {
            id: AtlasTextureId {
                index: index.unwrap_or(texture_list.textures.len()) as u32,
                kind,
            },
            allocator: BucketedAtlasAllocator::new(device_size_to_etagere(size)),
            format,
            texture,
            view,
            live_atlas_keys: 0,
        };

        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list
                .textures
                .get_mut(ix)
                .and_then(|t| t.as_mut())
                .expect("texture must exist")
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list
                .textures
                .last_mut()
                .and_then(|t| t.as_mut())
                .expect("texture must exist")
        }
    }

    fn upload_texture(&mut self, id: AtlasTextureId, bounds: Bounds<DevicePixels>, bytes: &[u8]) {
        let data = self
            .storage
            .get(id)
            .map(|texture| swizzle_upload_data(bytes, texture.format))
            .unwrap_or_else(|| bytes.to_vec());

        self.pending_uploads
            .push(PendingUpload { id, bounds, data });
    }

    fn flush_uploads(&mut self) {
        for upload in self.pending_uploads.drain(..) {
            let Some(texture) = self.storage.get(upload.id) else {
                continue;
            };
            let bytes_per_pixel = texture.bytes_per_pixel();

            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: upload.bounds.origin.x.0 as u32,
                        y: upload.bounds.origin.y.0 as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &upload.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(upload.bounds.size.width.0 as u32 * bytes_per_pixel as u32),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: upload.bounds.size.width.0 as u32,
                    height: upload.bounds.size.height.0 as u32,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

#[derive(Default)]
struct WgpuAtlasStorage {
    monochrome_textures: AtlasTextureList<WgpuAtlasTexture>,
    subpixel_textures: AtlasTextureList<WgpuAtlasTexture>,
    polychrome_textures: AtlasTextureList<WgpuAtlasTexture>,
}

impl ops::Index<AtlasTextureKind> for WgpuAtlasStorage {
    type Output = AtlasTextureList<WgpuAtlasTexture>;
    fn index(&self, kind: AtlasTextureKind) -> &Self::Output {
        match kind {
            AtlasTextureKind::Monochrome => &self.monochrome_textures,
            AtlasTextureKind::Subpixel => &self.subpixel_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
        }
    }
}

impl ops::IndexMut<AtlasTextureKind> for WgpuAtlasStorage {
    fn index_mut(&mut self, kind: AtlasTextureKind) -> &mut Self::Output {
        match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Subpixel => &mut self.subpixel_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
        }
    }
}

impl WgpuAtlasStorage {
    fn get(&self, id: AtlasTextureId) -> Option<&WgpuAtlasTexture> {
        self[id.kind]
            .textures
            .get(id.index as usize)
            .and_then(|t| t.as_ref())
    }
}

impl ops::Index<AtlasTextureId> for WgpuAtlasStorage {
    type Output = WgpuAtlasTexture;
    fn index(&self, id: AtlasTextureId) -> &Self::Output {
        let textures = match id.kind {
            AtlasTextureKind::Monochrome => &self.monochrome_textures,
            AtlasTextureKind::Subpixel => &self.subpixel_textures,
            AtlasTextureKind::Polychrome => &self.polychrome_textures,
        };
        textures[id.index as usize]
            .as_ref()
            .expect("texture must exist")
    }
}

struct WgpuAtlasTexture {
    id: AtlasTextureId,
    allocator: BucketedAtlasAllocator,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    live_atlas_keys: u32,
}

impl WgpuAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let allocation = self.allocator.allocate(device_size_to_etagere(size))?;
        let tile = AtlasTile {
            texture_id: self.id,
            tile_id: allocation.id.into(),
            padding: 0,
            bounds: Bounds {
                origin: etagere_point_to_device(allocation.rectangle.min),
                size,
            },
        };
        self.live_atlas_keys += 1;
        Some(tile)
    }

    fn bytes_per_pixel(&self) -> u8 {
        match self.format {
            wgpu::TextureFormat::R8Unorm => 1,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm => 4,
            _ => 4,
        }
    }

    fn decrement_ref_count(&mut self) {
        self.live_atlas_keys -= 1;
    }

    fn is_unreferenced(&self) -> bool {
        self.live_atlas_keys == 0
    }
}

fn swizzle_upload_data(bytes: &[u8], format: wgpu::TextureFormat) -> Vec<u8> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => {
            let mut data = bytes.to_vec();
            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            data
        }
        _ => bytes.to_vec(),
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use rgpui::block_on;
    use rgpui::{ImageId, RenderImageParams};
    use std::sync::Arc;

    fn test_device_and_queue() -> anyhow::Result<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
        block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                flags: wgpu::InstanceFlags::default(),
                backend_options: wgpu::BackendOptions::default(),
                memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                display: None,
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|error| anyhow::anyhow!("failed to request adapter: {error}"))?;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("wgpu_atlas_test_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits())
                        .using_alignment(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                })
                .await
                .map_err(|error| anyhow::anyhow!("failed to request device: {error}"))?;
            Ok((Arc::new(device), Arc::new(queue)))
        })
    }

    #[test]
    fn before_frame_skips_uploads_for_removed_texture() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;

        let atlas = WgpuAtlas::new(device, queue, wgpu::TextureFormat::Bgra8Unorm);
        let key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(1),
            frame_index: 0,
        });
        let size = Size {
            width: DevicePixels(1),
            height: DevicePixels(1),
        };
        let mut build = || Ok(Some((size, Cow::Owned(vec![0, 0, 0, 255]))));

        // Regression test: before the fix, this panicked in flush_uploads
        atlas
            .get_or_insert_with(&key, &mut build)?
            .expect("tile should be created");
        atlas.remove(&key);
        atlas.before_frame();
        Ok(())
    }

    #[test]
    fn remove_deallocates_tile_space_for_reuse() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;
        let atlas = WgpuAtlas::new(device, queue, wgpu::TextureFormat::Bgra8Unorm);

        let small = Size {
            width: DevicePixels(64),
            height: DevicePixels(64),
        };
        let big = Size {
            width: DevicePixels(700),
            height: DevicePixels(700),
        };

        let make_key = |image_id: usize| {
            AtlasKey::Image(RenderImageParams {
                image_id: ImageId(image_id),
                frame_index: 0,
            })
        };
        let insert = |key: &AtlasKey, size: Size<DevicePixels>| {
            let byte_count = (size.width.0 as usize) * (size.height.0 as usize) * 4;
            atlas
                .get_or_insert_with(key, &mut || {
                    Ok(Some((size, Cow::Owned(vec![0u8; byte_count]))))
                })
                .expect("allocation should succeed")
                .expect("callback returns Some")
        };

        let keeper_key = make_key(1);
        let big_key_a = make_key(2);
        let big_key_b = make_key(3);

        let keeper_tile = insert(&keeper_key, small);
        let tile_a = insert(&big_key_a, big);
        assert_eq!(keeper_tile.texture_id, tile_a.texture_id);

        atlas.remove(&big_key_a);
        let tile_b = insert(&big_key_b, big);
        assert_eq!(tile_b.texture_id, keeper_tile.texture_id);
        Ok(())
    }

    #[test]
    fn swizzle_upload_data_preserves_bgra_uploads() {
        let input = vec![0x10, 0x20, 0x30, 0x40];
        assert_eq!(
            swizzle_upload_data(&input, wgpu::TextureFormat::Bgra8Unorm),
            input
        );
    }

    #[test]
    fn swizzle_upload_data_converts_bgra_to_rgba() {
        let input = vec![0x10, 0x20, 0x30, 0x40, 0xAA, 0xBB, 0xCC, 0xDD];
        assert_eq!(
            swizzle_upload_data(&input, wgpu::TextureFormat::Rgba8Unorm),
            vec![0x30, 0x20, 0x10, 0x40, 0xCC, 0xBB, 0xAA, 0xDD]
        );
    }

    /// 「纹理 → MAP_READ 缓冲 → 回读」在真实驱动上能否跑通的最小复现。
    ///
    /// 刻意不建 surface、不做渲染 pass：如果这里都卡住，问题出在回读写法本身，
    /// 与窗口的交换链无关。需要 GPU，故 `#[ignore]`，本机用
    /// `cargo test -p rgpui-wgpu --lib -- --ignored --nocapture readback` 跑。
    #[test]
    #[ignore = "需要可用 GPU"]
    fn texture_readback_via_map_async_completes() -> anyhow::Result<()> {
        let (device, queue) = test_device_and_queue()?;

        // bytes_per_row 必须是 COPY_BYTES_PER_ROW_ALIGNMENT(256) 的整数倍，取 64 像素宽
        let width = 64u32;
        let height = 2u32;
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("readback_src"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let src: Vec<u8> = (0..width * height)
            .flat_map(|i| [i as u8, 0, 0, 255])
            .collect();
        queue.write_texture(
            texture.as_image_copy(),
            &src,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            extent,
        );

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback_dst"),
            size: (width * height * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback_copy"),
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfoBase {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            extent,
        );
        queue.submit(std::iter::once(encoder.finish()));

        let (sender, receiver) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        // 只用 `PollType::Poll` 自旋等回调：`Wait` 在软件渲染上可能立刻返回 `Timeout`
        // 且不推进围栏（见 `WgpuContext::probe_offscreen_readback` 的说明）。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match receiver.try_recv() {
                Ok(result) => {
                    result.map_err(|error| anyhow::anyhow!("{error}"))?;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    anyhow::bail!("回读回调没有触发");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if std::time::Instant::now() >= deadline {
                        anyhow::bail!("等待回读超时");
                    }
                    device.poll(wgpu::PollType::Poll)?;
                }
            }
        }

        let mapped = readback.slice(..).get_mapped_range()?;
        let bytes = mapped.to_vec();
        drop(mapped);
        readback.unmap();
        assert_eq!(bytes, src);
        Ok(())
    }
}
