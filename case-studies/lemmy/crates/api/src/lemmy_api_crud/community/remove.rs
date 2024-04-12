use crate::lemmy_api_common::{
    community::{CommunityResponse, RemoveCommunity},
    utils::{apply_label_community_write, blocking, get_local_user_view_from_jwt, is_admin, check_community_ban, check_community_deleted_or_removed},
};
use crate::lemmy_api_crud::PerformCrud;
use crate::lemmy_apub::activities::deletion::{send_apub_delete_in_community, DeletableObjects};
use crate::lemmy_db_schema::{
    source::{
        community::Community,
        moderator::{ModRemoveCommunity, ModRemoveCommunityForm},
    },
    traits::Crud,
};
use crate::lemmy_utils::{error::LemmyError, utils::naive_from_unix, ConnectionId};
use crate::lemmy_websocket::{send::send_community_ws_message, LemmyContext, UserOperationCrud};
use actix_web::web::Data;
use cfg_if::cfg_if;

#[async_trait::async_trait(?Send)]
impl PerformCrud for RemoveCommunity {
    type Response = CommunityResponse;

    #[tracing::instrument(skip(context, websocket_id))]
    #[cfg_attr(feature = "community-remove", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        websocket_id: Option<ConnectionId>,
    ) -> Result<CommunityResponse, LemmyError> {
        let data: &RemoveCommunity = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        // Verify its an admin (only an admin can remove a community)
        is_admin(&local_user_view)?;

        // Do the remove
        let community_id = data.community_id;
        let removed = data.removed;

        cfg_if! {
            if #[cfg(feature = "hypothetical-fix")] {
                check_community_ban(
                    local_user_view.person.id,
                    community_id,
                    context.pool(),
                )
                .await?;
                check_community_deleted_or_removed(community_id, context.pool()).await?;
            }
        }

        let updated_community = apply_label_community_write(
            blocking(context.pool(), move |conn| {
                Community::update_removed(conn, community_id, removed)
            })
            .await?,
        )
        .map_err(|e| LemmyError::from_error_message(e, "couldnt_update_community"))?;

        // Mod tables
        let expires = data.expires.map(naive_from_unix);
        let form = ModRemoveCommunityForm {
            mod_person_id: local_user_view.person.id,
            community_id: data.community_id,
            removed: Some(removed),
            reason: data.reason.to_owned(),
            expires,
        };
        apply_label_community_write(
            blocking(context.pool(), move |conn| {
                ModRemoveCommunity::create(conn, &form)
            })
            .await??,
        );

        let res = send_community_ws_message(
            data.community_id,
            UserOperationCrud::RemoveCommunity,
            websocket_id,
            Some(local_user_view.person.id),
            context,
        )
        .await?;

        // Apub messages
        let deletable = DeletableObjects::Community(Box::new(updated_community.clone().into()));
        send_apub_delete_in_community(
            local_user_view.person,
            updated_community,
            deletable,
            data.reason.clone().or_else(|| Some("".to_string())),
            removed,
            context,
        )
        .await?;
        Ok(res)
    }
}
