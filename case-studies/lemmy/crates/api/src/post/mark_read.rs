use crate::lemmy_api_common::{
    post::{MarkPostAsRead, PostResponse},
    utils::{
        apply_label_community_write, apply_label_read, blocking, get_local_user_view_from_jwt,
        mark_post_as_read, mark_post_as_unread, check_community_deleted_or_removed, check_community_ban
    },
};
use crate::lemmy_db_views::structs::PostView;
use crate::lemmy_db_schema::source::post::Post;
use crate::lemmy_db_schema::traits::Crud;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;
use crate::Perform;
use actix_web::web::Data;
use cfg_if::cfg_if;

#[async_trait::async_trait(?Send)]
impl Perform for MarkPostAsRead {
    type Response = PostResponse;

    #[tracing::instrument(skip(context, _websocket_id))]
    #[cfg_attr(feature = "post-mark-read", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        _websocket_id: Option<ConnectionId>,
    ) -> Result<Self::Response, LemmyError> {
        let data = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        let post_id = data.post_id;
        let person_id = local_user_view.person.id;

        cfg_if! {
            if #[cfg(feature = "hypothetical-fix")] {
                let orig_post = apply_label_read(
                    blocking(context.pool(), move |conn| Post::read(conn, post_id)).await??,
                );
                check_community_ban(
                    local_user_view.person.id,
                    orig_post.community_id,
                    context.pool(),
                )
                .await?;
                check_community_deleted_or_removed(orig_post.community_id, context.pool()).await?;
            }
        }

        // Mark the post as read / unread
        if data.read {
            apply_label_community_write(
                mark_post_as_read(person_id, post_id, context.pool()).await?,
            );
        } else {
            apply_label_community_write(
                mark_post_as_unread(person_id, post_id, context.pool()).await?,
            );
        }

        // Fetch it
        let post_view = apply_label_read(
            blocking(context.pool(), move |conn| {
                PostView::read(conn, post_id, Some(person_id))
            })
            .await??,
        );

        let res = Self::Response { post_view };

        Ok(res)
    }
}
