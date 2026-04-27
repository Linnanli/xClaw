pub const REVIEW_STATUS_PENDING: &str = "pending";
pub const REVIEW_STATUS_APPROVED: &str = "approved";
pub const REVIEW_STATUS_SCANNING: &str = "scanning";
pub const REVIEW_STATUS_SCAN_FAILED: &str = "scan_failed";
pub const REVIEW_STATUS_YANKED: &str = "yanked";

pub fn review_status_after_rescan(current_status: &str, is_safe: bool) -> &'static str {
    if !is_safe {
        return REVIEW_STATUS_SCAN_FAILED;
    }

    if current_status == REVIEW_STATUS_APPROVED {
        REVIEW_STATUS_APPROVED
    } else {
        REVIEW_STATUS_PENDING
    }
}

pub fn enabled_after_rescan(current_enabled: bool, current_status: &str, is_safe: bool) -> bool {
    is_safe && current_status == REVIEW_STATUS_APPROVED && current_enabled
}
