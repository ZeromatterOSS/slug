
load("@rules_cc//cc/private:cc_common.bzl", _cc_common = "cc_common")
load("@rules_cc//cc/private:cc_info.bzl", _CcInfo = "CcInfo")
load("@rules_cc//cc/private:cc_shared_library_info.bzl", _CcSharedLibraryInfo = "CcSharedLibraryInfo")
load("@rules_cc//cc/private:debug_package_info.bzl", _DebugPackageInfo = "DebugPackageInfo")
load("@rules_cc//cc/private:objc_info.bzl", _ObjcInfo = "ObjcInfo")
load("@rules_cc//cc/private/toolchain_config:cc_toolchain_config_info.bzl", _CcToolchainConfigInfo = "CcToolchainConfigInfo")

cc_common = _cc_common
CcInfo = _CcInfo
DebugPackageInfo = _DebugPackageInfo
CcToolchainConfigInfo = _CcToolchainConfigInfo
ObjcInfo = _ObjcInfo
new_objc_provider = _ObjcInfo
CcSharedLibraryInfo = _CcSharedLibraryInfo
            