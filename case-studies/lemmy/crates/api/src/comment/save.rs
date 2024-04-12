use crate::lemmy_api_common::{
    comment::{CommentResponse, SaveComment},
    utils::{
        apply_label_community_write, apply_label_read, blocking, get_local_user_view_from_jwt,
    },
};
use crate::lemmy_db_schema::{
    source::comment::{CommentSaved, CommentSavedForm},
    traits::Saveable,
};
use crate::lemmy_db_views::structs::CommentView;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;
use crate::Perform;
use actix_web::web::Data;

#[async_trait::async_trait(?Send)]
impl Perform for SaveComment {
    type Response = CommentResponse;

    #[tracing::instrument(skip(context, _websocket_id))]
    #[cfg_attr(feature = "comment-save", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        _websocket_id: Option<ConnectionId>,
    ) -> Result<CommentResponse, LemmyError> {
        let data: &SaveComment = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        let comment_saved_form = CommentSavedForm {
            comment_id: data.comment_id,
            person_id: local_user_view.person.id,
        };

        if #[cfg(feature = "hypothetical-fix")] {
            let comment_id = data.comment_id;
            let orig_comment = apply_label_read(
                blocking(context.pool(), move |conn| {
                    CommentView::read(conn, comment_id, None)
                })
                .await??,
            );

            check_community_ban(
                local_user_view.person.id,
                orig_comment.community.id,
                context.pool(),
            )
            .await?;
            check_community_deleted_or_removed(orig_comment.community.id, context.pool()).await?;
        }

        if data.save {
            let save_comment = move |conn: &'_ _| CommentSaved::save(conn, &comment_saved_form);
            apply_label_community_write(
                blocking(context.pool(), save_comment)
                    .await?
                    .map_err(|e| LemmyError::from_error_message(e, "couldnt_save_comment"))?,
            );
        } else {
            let unsave_comment = move |conn: &'_ _| CommentSaved::unsave(conn, &comment_saved_form);
            apply_label_community_write(
                blocking(context.pool(), unsave_comment)
                    .await?
                    .map_err(|e| LemmyError::from_error_message(e, "couldnt_save_comment"))?,
            );
        }

        let comment_id = data.comment_id;
        let person_id = local_user_view.person.id;
        let comment_view = apply_label_read(
            blocking(context.pool(), move |conn| {
                CommentView::read(conn, comment_id, Some(person_id))
            })
            .await??,
        );

        Ok(CommentResponse {
            comment_view,
            recipient_ids: Vec::new(),
            form_id: None,
        })
    }
}
