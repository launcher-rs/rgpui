use anyhow::{Context, Result};
use itertools::Itertools;
use rgpui::util::ResultExt;
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::{
            D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
        },
        Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG,
            D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS, D3D11_FEATURE_DATA_D3D10_X_HARDWARE_OPTIONS,
            D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
        },
        Dxgi::{
            CreateDXGIFactory2, DXGI_CREATE_FACTORY_DEBUG, DXGI_CREATE_FACTORY_FLAGS,
            IDXGIAdapter1, IDXGIFactory6,
        },
    },
};
use windows::core::Interface;

/// 尝试从设备丢失状态中恢复，最多重试 5 次
///
/// # 参数
/// * `f` - 需要执行的闭包，返回 Result<T>
///
/// # 返回
/// 成功时返回 Ok(T)，失败时返回错误
pub(crate) fn try_to_recover_from_device_lost<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    (0..5)
        .map(|i| {
            if i > 0 {
                // 重试前添加短暂延迟
                std::thread::sleep(std::time::Duration::from_millis(100 + i * 10));
            }
            f()
        })
        .find_or_last(Result::is_ok)
        .unwrap()
        .context("DirectXRenderer failed to recover from lost device after multiple attempts")
}

/// DirectX 设备集合
///
/// 封装了 Direct3D 11 和 DXGI 所需的核心设备对象
#[derive(Clone)]
pub(crate) struct DirectXDevices {
    /// DXGI 适配器
    pub(crate) adapter: IDXGIAdapter1,
    /// DXGI 工厂
    pub(crate) dxgi_factory: IDXGIFactory6,
    /// D3D11 设备
    pub(crate) device: ID3D11Device,
    /// D3D11 设备上下文
    pub(crate) device_context: ID3D11DeviceContext,
}

impl DirectXDevices {
    /// 创建新的 DirectX 设备实例
    ///
    /// # 返回
    /// 返回初始化成功的 DirectXDevices 实例，或错误
    pub(crate) fn new() -> Result<Self> {
        let debug_layer_available = check_debug_layer_available();
        let dxgi_factory =
            get_dxgi_factory(debug_layer_available).context("Creating DXGI factory")?;
        let (adapter, device, device_context, feature_level) =
            get_adapter(&dxgi_factory, debug_layer_available).context("Getting DXGI adapter")?;
        match feature_level {
            D3D_FEATURE_LEVEL_11_1 => {
                log::info!("创建了 Direct3D 11.1 特性级别的设备。")
            }
            D3D_FEATURE_LEVEL_11_0 => {
                log::info!("创建了 Direct3D 11.0 特性级别的设备。")
            }
            D3D_FEATURE_LEVEL_10_1 => {
                log::info!("创建了 Direct3D 10.1 特性级别的设备。")
            }
            _ => unreachable!(),
        }

        Ok(Self {
            adapter,
            dxgi_factory,
            device,
            device_context,
        })
    }
}

/// 检查调试层是否可用（仅在 debug 模式下）
#[inline]
fn check_debug_layer_available() -> bool {
    #[cfg(debug_assertions)]
    {
        use windows::Win32::Graphics::Dxgi::{DXGIGetDebugInterface1, IDXGIInfoQueue};

        unsafe { DXGIGetDebugInterface1::<IDXGIInfoQueue>(0) }
            .log_err()
            .is_some()
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

/// 获取 DXGI 工厂实例
///
/// # 参数
/// * `debug_layer_available` - 调试层是否可用
///
/// # 返回
/// 返回 DXGI 工厂实例，或错误
#[inline]
fn get_dxgi_factory(debug_layer_available: bool) -> Result<IDXGIFactory6> {
    let factory_flag = if debug_layer_available {
        DXGI_CREATE_FACTORY_DEBUG
    } else {
        #[cfg(debug_assertions)]
            log::warn!("获取 DXGI 调试接口失败。DirectX 调试功能将被禁用。");
        DXGI_CREATE_FACTORY_FLAGS::default()
    };
    unsafe { Ok(CreateDXGIFactory2(factory_flag)?) }
}

/// 枚举并获取合适的 DXGI 适配器
///
/// 遍历所有适配器，找到支持 Direct3D 11 的设备并创建它
///
/// # 参数
/// * `dxgi_factory` - DXGI 工厂实例
/// * `debug_layer_available` - 调试层是否可用
///
/// # 返回
/// 返回元组：(适配器, 设备, 设备上下文, 特性级别)
#[inline]
fn get_adapter(
    dxgi_factory: &IDXGIFactory6,
    debug_layer_available: bool,
) -> Result<(
    IDXGIAdapter1,
    ID3D11Device,
    ID3D11DeviceContext,
    D3D_FEATURE_LEVEL,
)> {
    for adapter_index in 0.. {
        let adapter: IDXGIAdapter1 = unsafe { dxgi_factory.EnumAdapters(adapter_index)?.cast()? };
        if let Ok(desc) = unsafe { adapter.GetDesc1() } {
            let gpu_name = String::from_utf16_lossy(&desc.Description)
                .trim_matches(char::from(0))
                .to_string();
            log::info!("Using GPU: {}", gpu_name);
        }
        // 检查适配器是否支持 Direct3D 11，如果支持则创建设备
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL::default();
        if let Some(device) = get_device(
            &adapter,
            Some(&mut context),
            Some(&mut feature_level),
            debug_layer_available,
        )
        .log_err()
        {
            return Ok((adapter, device, context.unwrap(), feature_level));
        }
    }

    unreachable!()
}

/// 在指定适配器上创建 D3D11 设备
///
/// # 参数
/// * `adapter` - DXGI 适配器
/// * `context` - 用于接收设备上下文的可选指针
/// * `feature_level` - 用于接收特性级别的可选指针
/// * `debug_layer_available` - 调试层是否可用
///
/// # 返回
/// 返回创建的 ID3D11Device 实例，或错误
#[inline]
fn get_device(
    adapter: &IDXGIAdapter1,
    context: Option<*mut Option<ID3D11DeviceContext>>,
    feature_level: Option<*mut D3D_FEATURE_LEVEL>,
    debug_layer_available: bool,
) -> Result<ID3D11Device> {
    let mut device: Option<ID3D11Device> = None;
    let device_flags = if debug_layer_available {
        D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_DEBUG
    } else {
        D3D11_CREATE_DEVICE_BGRA_SUPPORT
    };
    unsafe {
        D3D11CreateDevice(
            adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            device_flags,
            // Direct3D 特性级别 10.1 或更高需要 4x MSAA 支持
            Some(&[
                D3D_FEATURE_LEVEL_11_1,
                D3D_FEATURE_LEVEL_11_0,
                D3D_FEATURE_LEVEL_10_1,
            ]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            feature_level,
            context,
        )?;
    }
    let device = device.unwrap();
    let mut data = D3D11_FEATURE_DATA_D3D10_X_HARDWARE_OPTIONS::default();
    unsafe {
        device
            .CheckFeatureSupport(
                D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS,
                &mut data as *mut _ as _,
                std::mem::size_of::<D3D11_FEATURE_DATA_D3D10_X_HARDWARE_OPTIONS>() as u32,
            )
            .context("Checking GPU device feature support")?;
    }
    if data
        .ComputeShaders_Plus_RawAndStructuredBuffers_Via_Shader_4_x
        .as_bool()
    {
        Ok(device)
    } else {
        Err(anyhow::anyhow!(
            "GPU/驱动程序不支持所需的 StructuredBuffer 特性"
        ))
    }
}
