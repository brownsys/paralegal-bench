use anyhow::Result;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod eval_driver;
pub mod unhacked;

use paralegal_policy::{
    assert_warning,
    paralegal_spdg::{traverse::EdgeSelection, Identifier},
    Context, Diagnostics, Marker, PolicyContext, RootContext,
};

macro_rules! marker {
    ($id:ident) => {
        Marker::new_intern(stringify!($id))
    };
}

pub struct CommunityProp {
    cx: Arc<PolicyContext>,
}

pub struct InstanceProp {
    cx: Arc<PolicyContext>,
}

pub mod cnl {
    pub mod instance {
        include!(concat!(env!("OUT_DIR"), "/instance.rs"));
    }

    pub mod community {
        include!(concat!(env!("OUT_DIR"), "/community.rs"));
    }
}

impl CommunityProp {
    fn new(cx: Arc<PolicyContext>) -> Self {
        CommunityProp { cx }
    }

    fn check(&mut self) -> Result<()> {
        let ctx = &self.cx;
        let community_writes = self
            .cx
            .marked_nodes(marker!(db_community_write))
            .collect::<Box<[_]>>();
        let delete_check = marker!(community_delete_check);
        let ban_check = marker!(community_ban_check);

        assert_warning!(
            self.cx,
            !community_writes.is_empty(),
            "No writes found. The policy may be vacuous"
        );

        for write in community_writes.iter().copied() {
            let mut info_msg = self.cx.struct_node_note(write, "Found this write");
            if let Some(from) = ctx
                .marked_nodes(ban_check)
                .find(|n| ctx.has_ctrl_influence(*n, write))
            {
                info_msg.with_node_note(from, "This is its ban check");
            } else {
                ctx.node_error(write, "This write has no ban check")
            }
            if let Some(from) = ctx
                .marked_nodes(delete_check)
                .find(|n| ctx.has_ctrl_influence(*n, write))
            {
                info_msg.with_node_note(from, "This is its delete check");
            } else {
                ctx.node_error(write, "This write has no delete check")
            }
            info_msg.emit();
        }

        Ok(())
    }
}

impl InstanceProp {
    fn new(cx: Arc<PolicyContext>) -> Self {
        InstanceProp { cx }
    }

    fn check(&mut self) -> Result<()> {
        let ctx = &self.cx;
        let instance_delete = Identifier::new_intern("instance_delete_check");
        let instance_ban = Identifier::new_intern("instance_ban_check");
        let accesses = ctx
            .marked_nodes(Identifier::new_intern("db_access"))
            .filter(|n| !ctx.has_marker(Identifier::new_intern("db_user_read"), *n))
            .collect::<Vec<_>>();
        assert_warning!(
            self.cx,
            !accesses.is_empty(),
            "No accesses found. The policy may be vacuous"
        );

        for access in accesses {
            if !ctx
                .influencers(access, EdgeSelection::Both)
                .any(|n| ctx.has_marker(instance_delete, n))
            {
                ctx.node_error(access, "No delete check found for this access");
            }
            if !ctx
                .influencers(access, EdgeSelection::Both)
                .any(|n| ctx.has_marker(instance_ban, n))
            {
                ctx.node_error(access, "No ban check found for this access");
            }
        }

        Ok(())
    }
}

#[derive(
    ValueEnum, Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, strum::AsRefStr,
)]
#[strum(serialize_all = "kebab-case")]
pub enum Prop {
    Community,
    Instance,
}

impl Prop {
    pub fn run(
        self,
        cx: Arc<RootContext>,
        new_version: bool,
        verbose: bool,
        cnl: bool,
    ) -> anyhow::Result<()> {
        if cnl {
            assert!(!new_version);
            return match self {
                Self::Community => cnl::community::check(cx),
                Self::Instance => cnl::instance::check(cx),
            };
        }
        match self {
            Self::Community => cx.named_policy(Identifier::new_intern("Community Policy"), |cx| {
                if new_version {
                    unhacked::check_community(cx, verbose)
                } else {
                    CommunityProp::new(cx).check()
                }
            }),
            Self::Instance => cx.named_policy(Identifier::new_intern("Instance Policy"), |cx| {
                if new_version {
                    unhacked::check_instance(cx, verbose)
                } else {
                    InstanceProp::new(cx).check()
                }
            }),
        }
    }
}
