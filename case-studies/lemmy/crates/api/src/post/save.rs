use crate::lemmy_api_common::{
    post::{PostResponse, SavePost},
    utils::{
        apply_label_community_write, apply_label_read, blocking, get_local_user_view_from_jwt,
        mark_post_as_read, check_community_deleted_or_removed, check_community_ban
    },
};
use crate::lemmy_db_schema::{
    source::post::{Post, PostSaved, PostSavedForm},
    traits::{Crud, Saveable},
};
use crate::lemmy_db_views::structs::PostView;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;
use crate::Perform;
use actix_web::web::Data;
use cfg_if::cfg_if;

#[async_trait::async_trait(?Send)]
impl Perform for SavePost {
    type Response = PostResponse;

    #[tracing::instrument(skip(context, _websocket_id))]
    #[cfg_attr(feature = "post-save", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        _websocket_id: Option<ConnectionId>,
    ) -> Result<PostResponse, LemmyError> {
        let data: &SavePost = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        let post_saved_form = PostSavedForm {
            post_id: data.post_id,
            person_id: local_user_view.person.id,
        };

        cfg_if! {
            if #[cfg(feature = "hypothetical-fix")] {
                let post_id = data.post_id;
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

        if data.save {
            let save = move |conn: &'_ _| PostSaved::save(conn, &post_saved_form);
            apply_label_community_write(
                blocking(context.pool(), save)
                    .await?
                    .map_err(|e| LemmyError::from_error_message(e, "couldnt_save_post"))?,
            );
        } else {
            let unsave = move |conn: &'_ _| PostSaved::unsave(conn, &post_saved_form);
            apply_label_community_write(
                blocking(context.pool(), unsave)
                    .await?
                    .map_err(|e| LemmyError::from_error_message(e, "couldnt_save_post"))?,
            );
        }

        let post_id = data.post_id;
        let person_id = local_user_view.person.id;
        let post_view = apply_label_read(
            blocking(context.pool(), move |conn| {
                PostView::read(conn, post_id, Some(person_id))
            })
            .await??,
        );

        // Mark the post as read
        apply_label_community_write(mark_post_as_read(person_id, post_id, context.pool()).await?);

        Ok(PostResponse { post_view })
    }
}
