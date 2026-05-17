use std::borrow::Cow;
use std::sync::Arc;

use schemars::{JsonSchema, json_schema};

/// 可为给定字体配置的 OpenType 功能。
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct FontFeatures(pub Arc<Vec<(String, u32)>>);

impl FontFeatures {
    /// 禁用 `calt`（连字）功能。
    pub fn disable_ligatures() -> Self {
        Self(Arc::new(vec![("calt".into(), 0)]))
    }

    /// 获取字体 OpenType 功能的标签名称列表
    /// 仅返回已启用或禁用的功能
    pub fn tag_value_list(&self) -> &[(String, u32)] {
        self.0.as_slice()
    }

    /// 返回 `calt` 功能是否已启用。
    ///
    /// 如果该功能不存在，返回 `None`。
    pub fn is_calt_enabled(&self) -> Option<bool> {
        self.0
            .iter()
            .find(|(feature, _)| feature == "calt")
            .map(|(_, value)| *value == 1)
    }
}

impl std::fmt::Debug for FontFeatures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("FontFeatures");
        for (tag, value) in self.tag_value_list() {
            debug.field(tag, value);
        }

        debug.finish()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum FeatureValue {
    Bool(bool),
    Number(serde_json::Number),
}

impl<'de> serde::Deserialize<'de> for FontFeatures {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct FontFeaturesVisitor;

        impl<'de> Visitor<'de> for FontFeaturesVisitor {
            type Value = FontFeatures;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("字体功能映射")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut feature_list = Vec::new();

                while let Some((key, value)) =
                    access.next_entry::<String, Option<FeatureValue>>()?
                {
                    if !is_valid_feature_tag(&key) {
                        log::error!("Incorrect font feature tag: {}", key);
                        continue;
                    }
                    if let Some(value) = value {
                        match value {
                            FeatureValue::Bool(enable) => {
                                if enable {
                                    feature_list.push((key, 1));
                                } else {
                                    feature_list.push((key, 0));
                                }
                            }
                            FeatureValue::Number(value) => {
                                if value.is_u64() {
                                    feature_list.push((key, value.as_u64().unwrap() as u32));
                                } else {
                                    log::error!(
                                        "Incorrect font feature value {} for feature tag {}",
                                        value,
                                        key
                                    );
                                    continue;
                                }
                            }
                        }
                    }
                }

                Ok(FontFeatures(Arc::new(feature_list)))
            }
        }

        let features = deserializer.deserialize_map(FontFeaturesVisitor)?;
        Ok(features)
    }
}

impl serde::Serialize for FontFeatures {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;

        for (tag, value) in self.tag_value_list() {
            map.serialize_entry(tag, value)?;
        }

        map.end()
    }
}

impl JsonSchema for FontFeatures {
    fn schema_name() -> Cow<'static, str> {
        "FontFeatures".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        json_schema!({
            "type": "object",
            "patternProperties": {
                "[0-9a-zA-Z]{4}$": {
                    "type": ["boolean", "integer"],
                    "minimum": 0,
                    "multipleOf": 1
                }
            },
            "additionalProperties": false
        })
    }
}

/// 验证字体功能标签是否有效（必须为4个ASCII字母数字字符）
fn is_valid_feature_tag(tag: &str) -> bool {
    tag.len() == 4 && tag.chars().all(|c| c.is_ascii_alphanumeric())
}
