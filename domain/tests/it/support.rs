use domain::Errors;
use domain::features::{Call, Origin, Target};
use serde_json::{Error, from_str};

pub(crate) const ERRORS: &str = r#"{"bad_request":{"code":"invalid","exit_code":13},"unknown_action":{"code":"missing","exit_code":14}}"#;

pub(crate) const fn cli<'call>(
    feature: &'call str,
    action: &'call str,
    request: &'call str,
) -> Call<'call> {
    Call::new(
        Origin::new("cli", None, None),
        Target::new(feature, action),
        request,
    )
}

pub(crate) fn errors() -> Result<Errors, Error> {
    from_str(ERRORS)
}
