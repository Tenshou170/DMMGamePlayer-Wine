use windows::core::PCWSTR;

macro_rules! define_id {
    ($name:ident, $value:tt) => (
        pub const $name: PCWSTR = PCWSTR($value as u16 as *const u16);
    )
}

#[allow(dead_code)]
macro_rules! define_idc {
    ($name:ident, $value:tt) => (
        #[allow(dead_code)]
        pub const $name: i32 = $value;
    )
}

// Dialog
define_id!(IDD_MAIN, 129);

// Controls
define_idc!(IDC_TITLE_LABEL, 1000);
define_idc!(IDC_VERSION_LABEL, 1001);
define_idc!(IDC_INSTALLED_LABEL, 1002);
define_idc!(IDC_GROUP_ENV, 1003);
define_idc!(IDC_RUNNER_STATUS, 1004);
define_idc!(IDC_PREFIX_STATUS, 1005);
define_idc!(IDC_DIAGNOSTICS_STATUS, 1006);
define_idc!(IDC_GROUP_PATH, 1007);
define_idc!(IDC_INSTALL_PATH, 1008);
define_idc!(IDC_INSTALL_PATH_BROWSE, 1009);
define_idc!(IDC_STATUS, 1010);
define_idc!(IDC_PROGRESS, 1011);
define_idc!(IDC_INSTALL, 1012);
define_idc!(IDC_REPAIR, 1013);
define_idc!(IDC_UNINSTALL, 1014);

// Icons
define_id!(IDI_DMM, 107);


