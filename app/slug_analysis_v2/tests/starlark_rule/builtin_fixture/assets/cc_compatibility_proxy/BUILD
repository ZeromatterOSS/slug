
load("@bazel_skylib//:bzl_library.bzl", "bzl_library")
bzl_library(
  name = "proxy_bzl",
  srcs = ["proxy.bzl"],
  deps = [
    "@rules_cc//cc/private/rules_impl:core_rules",
    "@rules_cc//cc/private/rules_impl:toolchain_rules",
    "@rules_cc//cc/private:cc_common",
  ],
  visibility = ["@rules_cc//cc:__subpackages__"],
)
bzl_library(
  name = "symbols_bzl",
  srcs = ["symbols.bzl"],
  deps = [
    "@rules_cc//cc/private:cc_common",
    "@rules_cc//cc/private:cc_shared_library_info_bzl",
    "@rules_cc//cc/private/toolchain_config:toolchain_config_bzl",
  ],
  visibility = ["@rules_cc//cc:__subpackages__"],
)
            