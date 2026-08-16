use rgpui::Window;

/// [`Input`](super::Input) 的语义内容类型。
///
/// 这些变体对应 Swift 的文本内容类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputContentType {
    /// 人的全名。
    Name,
    /// 姓名前缀，如 Mr. 或 Dr.。
    NamePrefix,
    /// 人的名字。
    GivenName,
    /// 人的中间名。
    MiddleName,
    /// 人的姓氏。
    FamilyName,
    /// 姓名后缀，如 Jr. 或 PhD.。
    NameSuffix,
    /// 昵称。
    Nickname,
    /// 职位名称。
    JobTitle,
    /// 组织或公司名称。
    OrganizationName,
    /// 地点名称。
    Location,
    /// 完整街道地址。
    FullStreetAddress,
    /// 街道地址第一行。
    StreetAddressLine1,
    /// 街道地址第二行。
    StreetAddressLine2,
    /// 城市或地区。
    AddressCity,
    /// 州、省或地区。
    AddressState,
    /// 城市与州的组合。
    AddressCityAndState,
    /// 子地区、区或社区。
    Sublocality,
    /// 国家名称。
    CountryName,
    /// 邮政编码。
    PostalCode,
    /// 电话号码。
    TelephoneNumber,
    /// 电子邮件地址。
    EmailAddress,
    /// URL。
    Url,
    /// 信用卡号。
    CreditCardNumber,
    /// 信用卡上的全名。
    CreditCardName,
    /// 信用卡上的名字。
    CreditCardGivenName,
    /// 信用卡上的中间名。
    CreditCardMiddleName,
    /// 信用卡上的姓氏。
    CreditCardFamilyName,
    /// 信用卡安全码。
    CreditCardSecurityCode,
    /// 信用卡到期日期。
    CreditCardExpiration,
    /// 信用卡到期月份。
    CreditCardExpirationMonth,
    /// 信用卡到期年份。
    CreditCardExpirationYear,
    /// 信用卡类型。
    CreditCardType,
    /// 用户名或账户标识符。
    Username,
    /// 账户对应的密码。
    Password,
    /// 新密码，如注册或密码重置期间。
    NewPassword,
    /// 一次性验证码。
    OneTimeCode,
    /// 包裹运输追踪号。
    ShipmentTrackingNumber,
    /// 航班号。
    FlightNumber,
    /// 日期、时间或时长。
    DateTime,
    /// 出生日期。
    Birthdate,
    /// 出生日期（日）。
    BirthdateDay,
    /// 出生日期（月）。
    BirthdateMonth,
    /// 出生日期（年）。
    BirthdateYear,
    /// eSIM EID。
    CellularEid,
    /// 蜂窝 IMEI。
    CellularImei,
}

impl InputContentType {
    #[cfg(target_os = "macos")]
    pub(crate) const fn ns_text_content_type(self) -> Option<&'static str> {
        match self {
            Self::Name => Some("name"),
            Self::NamePrefix => Some("honorific-prefix"),
            Self::GivenName => Some("given-name"),
            Self::MiddleName => Some("additional-name"),
            Self::FamilyName => Some("family-name"),
            Self::NameSuffix => Some("honorific-suffix"),
            Self::Nickname => Some("nickname"),
            Self::JobTitle => Some("organization-title"),
            Self::OrganizationName => Some("organization"),
            Self::Location => Some("location"),
            Self::FullStreetAddress => Some("street-address"),
            Self::StreetAddressLine1 => Some("address-line1"),
            Self::StreetAddressLine2 => Some("address-line2"),
            Self::AddressCity => Some("address-level2"),
            Self::AddressState => Some("address-level1"),
            Self::AddressCityAndState => Some("address-level1+2"),
            Self::Sublocality => Some("address-level3"),
            Self::CountryName => Some("country-name"),
            Self::PostalCode => Some("postal-code"),
            Self::TelephoneNumber => Some("tel"),
            Self::EmailAddress => Some("email"),
            Self::Url => Some("url"),
            Self::CreditCardNumber => Some("cc-number"),
            Self::CreditCardName => Some("cc-name"),
            Self::CreditCardGivenName => Some("cc-given-name"),
            Self::CreditCardMiddleName => Some("cc-additional-name"),
            Self::CreditCardFamilyName => Some("cc-family-name"),
            Self::CreditCardSecurityCode => Some("cc-csc"),
            Self::CreditCardExpiration => Some("cc-exp"),
            Self::CreditCardExpirationMonth => Some("cc-exp-month"),
            Self::CreditCardExpirationYear => Some("cc-exp-year"),
            Self::CreditCardType => Some("cc-type"),
            Self::Username => Some("username"),
            Self::Password => Some("password"),
            Self::NewPassword => Some("new-password"),
            Self::OneTimeCode => Some("one-time-code"),
            Self::ShipmentTrackingNumber => Some("shipment-tracking-number"),
            Self::FlightNumber => Some("flight-number"),
            Self::DateTime => Some("date-time"),
            Self::Birthdate => Some("bday"),
            Self::BirthdateDay => Some("bday-day"),
            Self::BirthdateMonth => Some("bday-month"),
            Self::BirthdateYear => Some("bday-year"),
            Self::CellularEid | Self::CellularImei => None,
        }
    }
}

/// 同步原生内容类型到平台窗口。
///
/// macOS 上设置输入法内容类型，其他平台为空操作。
pub(super) fn sync_native_content_type(
    window: &mut Window,
    content_type: Option<InputContentType>,
    disabled: bool,
) {
    if disabled {
        return;
    }

    #[cfg(target_os = "macos")]
    super::native::set_text_content_type(window, content_type);

    #[cfg(not(target_os = "macos"))]
    let _ = (window, content_type);
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn content_type_maps_to_ns_text_content_type_values() {
        let content_types = [
            (InputContentType::Name, Some("name")),
            (InputContentType::NamePrefix, Some("honorific-prefix")),
            (InputContentType::GivenName, Some("given-name")),
            (InputContentType::MiddleName, Some("additional-name")),
            (InputContentType::FamilyName, Some("family-name")),
            (InputContentType::NameSuffix, Some("honorific-suffix")),
            (InputContentType::Nickname, Some("nickname")),
            (InputContentType::JobTitle, Some("organization-title")),
            (InputContentType::OrganizationName, Some("organization")),
            (InputContentType::Location, Some("location")),
            (InputContentType::FullStreetAddress, Some("street-address")),
            (InputContentType::StreetAddressLine1, Some("address-line1")),
            (InputContentType::StreetAddressLine2, Some("address-line2")),
            (InputContentType::AddressCity, Some("address-level2")),
            (InputContentType::AddressState, Some("address-level1")),
            (
                InputContentType::AddressCityAndState,
                Some("address-level1+2"),
            ),
            (InputContentType::Sublocality, Some("address-level3")),
            (InputContentType::CountryName, Some("country-name")),
            (InputContentType::PostalCode, Some("postal-code")),
            (InputContentType::TelephoneNumber, Some("tel")),
            (InputContentType::EmailAddress, Some("email")),
            (InputContentType::Url, Some("url")),
            (InputContentType::CreditCardNumber, Some("cc-number")),
            (InputContentType::CreditCardName, Some("cc-name")),
            (InputContentType::CreditCardGivenName, Some("cc-given-name")),
            (
                InputContentType::CreditCardMiddleName,
                Some("cc-additional-name"),
            ),
            (
                InputContentType::CreditCardFamilyName,
                Some("cc-family-name"),
            ),
            (InputContentType::CreditCardSecurityCode, Some("cc-csc")),
            (InputContentType::CreditCardExpiration, Some("cc-exp")),
            (
                InputContentType::CreditCardExpirationMonth,
                Some("cc-exp-month"),
            ),
            (
                InputContentType::CreditCardExpirationYear,
                Some("cc-exp-year"),
            ),
            (InputContentType::CreditCardType, Some("cc-type")),
            (InputContentType::Username, Some("username")),
            (InputContentType::Password, Some("password")),
            (InputContentType::NewPassword, Some("new-password")),
            (InputContentType::OneTimeCode, Some("one-time-code")),
            (
                InputContentType::ShipmentTrackingNumber,
                Some("shipment-tracking-number"),
            ),
            (InputContentType::FlightNumber, Some("flight-number")),
            (InputContentType::DateTime, Some("date-time")),
            (InputContentType::Birthdate, Some("bday")),
            (InputContentType::BirthdateDay, Some("bday-day")),
            (InputContentType::BirthdateMonth, Some("bday-month")),
            (InputContentType::BirthdateYear, Some("bday-year")),
            (InputContentType::CellularEid, None),
            (InputContentType::CellularImei, None),
        ];

        for (content_type, native_value) in content_types {
            assert_eq!(content_type.ns_text_content_type(), native_value);
        }
    }
}
