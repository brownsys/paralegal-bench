use crate::lemmy_api_common::{
    community::{AddModToCommunity, AddModToCommunityResponse},
    utils::{
        apply_label_community_write, apply_label_read, blocking, get_local_user_view_from_jwt,
        is_mod_or_admin,
    },
};
use crate::lemmy_apub::{
    objects::{community::ApubCommunity, person::ApubPerson},
    protocol::activities::community::{add_mod::AddMod, remove_mod::RemoveMod},
};
use crate::lemmy_db_schema::{
    source::{
        community::{Community, CommunityModerator, CommunityModeratorForm},
        moderator::{ModAddCommunity, ModAddCommunityForm},
        person::Person,
    },
    traits::{Crud, Joinable},
};
use crate::lemmy_db_views_actor::structs::CommunityModeratorView;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::{messages::SendCommunityRoomMessage, LemmyContext, UserOperation};
use crate::Perform;
use actix_web::web::Data;

#[async_trait::async_trait(?Send)]
impl Perform for AddModToCommunity {
    type Response = AddModToCommunityResponse;

    #[tracing::instrument(skip(context, websocket_id))]
    #[cfg_attr(feature = "community-add-mod", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        websocket_id: Option<ConnectionId>,
    ) -> Result<AddModToCommunityResponse, LemmyError> {
        let data: &AddModToCommunity = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        let community_id = data.community_id;

        // Verify that only mods or admins can add mod
        is_mod_or_admin(context.pool(), local_user_view.person.id, community_id).await?;
        let community = apply_label_read(
            blocking(context.pool(), move |conn| {
                Community::read(conn, community_id)
            })
            .await??,
        );
        if local_user_view.person.admin && !community.local {
            return Err(LemmyError::from_message("not_a_moderator"));
        }

        if #[cfg(feature = "hypothetical-fix")] {
            check_community_ban(
                local_user_view.person.id,
                community_id,
                context.pool(),
            )
            .await?;
            check_community_deleted_or_removed(community_id, context.pool()).await?;
        }

        // Update in local database
        let community_moderator_form = CommunityModeratorForm {
            community_id: data.community_id,
            person_id: data.person_id,
        };
        if data.added {
            let join = move |conn: &'_ _| CommunityModerator::join(conn, &community_moderator_form);
            apply_label_community_write(blocking(context.pool(), join).await?.map_err(|e| {
                LemmyError::from_error_message(e, "community_moderator_already_exists")
            }))?;
        } else {
            let leave =
                move |conn: &'_ _| CommunityModerator::leave(conn, &community_moderator_form);
            apply_label_community_write(blocking(context.pool(), leave).await?.map_err(|e| {
                LemmyError::from_error_message(e, "community_moderator_already_exists")
            }))?;
        }

        // Mod tables
        let form = ModAddCommunityForm {
            mod_person_id: local_user_view.person.id,
            other_person_id: data.person_id,
            community_id: data.community_id,
            removed: Some(!data.added),
        };
        apply_label_community_write(
            blocking(context.pool(), move |conn| {
                ModAddCommunity::create(conn, &form)
            })
            .await??,
        );

        // Send to federated instances
        let updated_mod_id = data.person_id;
        let updated_mod: ApubPerson = apply_label_read(
            blocking(context.pool(), move |conn| {
                Person::read(conn, updated_mod_id)
            })
            .await??,
        )
        .into();
        let community: ApubCommunity = community.into();
        if data.added {
            AddMod::send(
                &community,
                &updated_mod,
                &local_user_view.person.into(),
                context,
            )
            .await?;
        } else {
            RemoveMod::send(
                &community,
                &updated_mod,
                &local_user_view.person.into(),
                context,
            )
            .await?;
        }

        // Note: in case a remote mod is added, this returns the old moderators list, it will only get
        //       updated once we receive an activity from the community (like `Announce/Add/Moderator`)
        let community_id = data.community_id;
        let moderators = apply_label_read(
            blocking(context.pool(), move |conn| {
                CommunityModeratorView::for_community(conn, community_id)
            })
            .await??,
        );

        let res = AddModToCommunityResponse { moderators };
        context.chat_server().do_send(SendCommunityRoomMessage {
            op: UserOperation::AddModToCommunity,
            response: res.clone(),
            community_id,
            websocket_id,
        });
        Ok(res)
    }
}
