mod field;
mod form;

pub use field::*;
pub use form::*;

/// 创建一个垂直布局的新表单。
pub fn v_form() -> Form {
    Form::vertical()
}

/// 创建一个水平布局的新表单。
pub fn h_form() -> Form {
    Form::horizontal()
}

/// 创建一个新的表单字段。
pub fn field() -> Field {
    Field::new()
}
