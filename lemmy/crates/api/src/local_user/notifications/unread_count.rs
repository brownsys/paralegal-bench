use crate::lemmy_api_common::{
    person::{GetUnreadCount, GetUnreadCountResponse},
    utils::{apply_label_read, blocking, get_local_user_view_from_jwt},
};
use crate::lemmy_db_views::structs::{CommentView, PrivateMessageView};
use crate::lemmy_db_views_actor::structs::PersonMentionView;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;
use crate::Perform;
use actix_web::web::Data;

#[async_trait::async_trait(?Send)]
impl Perform for GetUnreadCount {
    type Response = GetUnreadCountResponse;

    #[tracing::instrument(skip(context, _websocket_id))]
    #[cfg_attr(feature = "notification-unread-count", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        _websocket_id: Option<ConnectionId>,
    ) -> Result<Self::Response, LemmyError> {
        let data = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        let person_id = local_user_view.person.id;

        let replies = apply_label_read(
            blocking(context.pool(), move |conn| {
                CommentView::get_unread_replies(conn, person_id)
            })
            .await??,
        );

        let mentions = apply_label_read(
            blocking(context.pool(), move |conn| {
                PersonMentionView::get_unread_mentions(conn, person_id)
            })
            .await??,
        );

        let private_messages = apply_label_read(
            blocking(context.pool(), move |conn| {
                PrivateMessageView::get_unread_messages(conn, person_id)
            })
            .await??,
        );

        let res = Self::Response {
            replies,
            mentions,
            private_messages,
        };

        Ok(res)
    }
}
