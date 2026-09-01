use schemars::{JsonSchema, transform::transform_subschemas};

const DEFS_PATH: &str = "#/$defs/";

/// 替换某个类型在定义列表中的 JSON schema 定义（如果正在使用），并返回其引用。
///
/// 此处断言 `JsonSchema::schema_name() + "2"` 不存在，因为这意味着有多个类型使用了该名称，
/// 而 schemars API 不支持解决此歧义——参见 <https://github.com/GREsau/schemars/issues/449>
///
/// `schema` 参数为闭包，因为某些设置类型在远程服务器上不可用，
/// 访问时（如 GlobalThemeRegistry）会崩溃。
pub fn replace_subschema<T: JsonSchema>(
    generator: &mut schemars::SchemaGenerator,
    schema: impl Fn() -> schemars::Schema,
) -> schemars::Schema {
    let schema_name = T::schema_name();
    let definitions = generator.definitions_mut();
    assert!(!definitions.contains_key(&format!("{schema_name}2")));
    assert!(definitions.contains_key(schema_name.as_ref()));
    definitions.insert(schema_name.to_string(), schema().to_value());
    schemars::Schema::new_ref(format!("{DEFS_PATH}{schema_name}"))
}

/// 添加新的 JSON schema 定义并返回其引用。如果名称已被使用则 **panic**。
pub fn add_new_subschema(
    generator: &mut schemars::SchemaGenerator,
    name: &str,
    schema: serde_json::Value,
) -> schemars::Schema {
    let old_definition = generator.definitions_mut().insert(name.to_string(), schema);
    assert_eq!(old_definition, None);
    schemars::Schema::new_ref(format!("{DEFS_PATH}{name}"))
}

/// 将 `additionalProperties` 默认为 `true`，等效于每个结构体都标注了
/// `#[schemars(deny_unknown_fields)]`。跳过已设置 `additionalProperties` 的结构体
///（例如使用了 `#[serde(flatten)]` 的 map）。
#[derive(Clone)]
pub struct DefaultDenyUnknownFields;

impl schemars::transform::Transform for DefaultDenyUnknownFields {
    fn transform(&mut self, schema: &mut schemars::Schema) {
        if let Some(object) = schema.as_object_mut()
            && object.contains_key("properties")
            && !object.contains_key("additionalProperties")
            && !object.contains_key("unevaluatedProperties")
        {
            object.insert("additionalProperties".to_string(), false.into());
        }
        transform_subschemas(self, schema);
    }
}

/// 将 `allowTrailingCommas` 默认为 `true`，供 `json-language-server` 使用。
/// 可应用于任何将被视为 `jsonc` 的 schema。
///
/// 注意：此转换非递归，仅作用于根 schema。
#[derive(Clone)]
pub struct AllowTrailingCommas;

impl schemars::transform::Transform for AllowTrailingCommas {
    fn transform(&mut self, schema: &mut schemars::Schema) {
        if let Some(object) = schema.as_object_mut()
            && !object.contains_key("allowTrailingCommas")
        {
            object.insert("allowTrailingCommas".to_string(), true.into());
        }
    }
}
