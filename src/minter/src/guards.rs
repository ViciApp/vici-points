use candid::Principal;
use ic_cdk::api::{is_controller, msg_caller};

/// Guard function to ensure the caller is not anonymous.
pub(crate) fn caller_is_not_anonymous() -> Result<(), String> {
    let c = msg_caller();
    if c == Principal::anonymous() {
        Err(String::from("anonymous callers are not allowed"))
    } else {
        Ok(())
    }
}

/// Guard function to ensure the caller is one of the canister controllers.
pub(crate) fn caller_is_controller() -> Result<(), String> {
    let c = msg_caller();
    if is_controller(&c) {
        Ok(())
    } else {
        Err(String::from("caller is not a controller"))
    }
}
