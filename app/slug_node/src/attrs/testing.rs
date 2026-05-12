/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::sync::Arc;

use dupe::Dupe;
use slug_core::configuration::config_setting::ConfigSettingData;
use slug_core::configuration::data::ConfigurationData;
use slug_core::configuration::data::ConfigurationDataData;
use slug_core::configuration::pair::ConfigurationNoExec;
use slug_core::configuration::pair::ConfigurationWithExec;
use slug_core::configuration::transition::applied::TransitionApplied;
use slug_core::configuration::transition::id::TransitionId;
use slug_core::target::label::label::TargetLabel;
use starlark_map::ordered_map::OrderedMap;
use starlark_map::unordered_map::UnorderedMap;

use crate::attrs::configuration_context::AttrConfigurationContext;
use crate::configuration::resolved::ConfigurationNode;
use crate::configuration::resolved::ConfigurationSettingKey;
use crate::configuration::resolved::MatchedConfigurationSettingKeys;

pub fn configuration_ctx() -> impl AttrConfigurationContext {
    struct TestAttrConfigurationContext(
        ConfigurationData,
        ConfigurationData,
        MatchedConfigurationSettingKeys,
    );
    impl AttrConfigurationContext for TestAttrConfigurationContext {
        fn cfg(&self) -> ConfigurationNoExec {
            ConfigurationNoExec::new(self.0.dupe())
        }

        fn exec_cfg(&self) -> slug_error::Result<ConfigurationNoExec> {
            Ok(ConfigurationNoExec::new(self.1.dupe()))
        }

        fn matched_cfg_keys(&self) -> &MatchedConfigurationSettingKeys {
            &self.2
        }

        fn toolchain_cfg(&self) -> ConfigurationWithExec {
            ConfigurationWithExec::new(self.0.dupe(), self.1.dupe())
        }

        fn platform_cfg(&self, _label: &TargetLabel) -> slug_error::Result<ConfigurationData> {
            panic!("not used in tests")
        }

        fn resolved_transitions(
            &self,
        ) -> slug_error::Result<&OrderedMap<Arc<TransitionId>, Arc<TransitionApplied>>> {
            panic!("not used in tests")
        }
    }

    TestAttrConfigurationContext(
        ConfigurationData::testing_new(),
        ConfigurationData::from_platform(
            "cfg_for//:testing_exec".to_owned(),
            ConfigurationDataData::empty(),
        )
        .unwrap(),
        MatchedConfigurationSettingKeys::new(UnorderedMap::from_iter([
            (
                ConfigurationSettingKey::testing_parse("root//other:config"),
                ConfigurationNode::new(Some(ConfigSettingData {
                    constraints: BTreeMap::new(),
                    buckconfigs: BTreeMap::new(),
                })),
            ),
            (
                ConfigurationSettingKey::testing_parse("root//some:config"),
                ConfigurationNode::new(None),
            ),
            (
                ConfigurationSettingKey::testing_parse("cell1//other:config"),
                ConfigurationNode::new(None),
            ),
        ])),
    )
}
