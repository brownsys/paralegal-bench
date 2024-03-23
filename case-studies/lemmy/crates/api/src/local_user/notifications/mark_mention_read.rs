use crate::lemmy_api_common::{
    person::{MarkPersonMentionAsRead, PersonMentionResponse},
    utils::{apply_label_read, apply_label_write, blocking, get_local_user_view_from_jwt},
};
use crate::lemmy_db_schema::{source::person_mention::PersonMention, traits::Crud};
use crate::lemmy_db_views_actor::structs::PersonMentionView;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;
use crate::Perform;
use actix_web::web::Data;

#[async_trait::async_trait(?Send)]
impl Perform for MarkPersonMentionAsRead {
    type Response = PersonMentionResponse;

    #[tracing::instrument(skip(context, _websocket_id))]
    #[cfg_attr(feature = "notification-mark-mention-read", paralegal::analyze)]
    async fn perform(
        &self,
        context: &Data<LemmyContext>,
        _websocket_id: Option<ConnectionId>,
    ) -> Result<PersonMentionResponse, LemmyError> {
        let data: &MarkPersonMentionAsRead = self;
        let local_user_view =
            get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

        let person_mention_id = data.person_mention_id;
        let read_person_mention = apply_label_read(
            blocking(context.pool(), move |conn| {
                PersonMention::read(conn, person_mention_id)
            })
            .await??,
        );

        if local_user_view.person.id != read_person_mention.recipient_id {
            return Err(LemmyError::from_message("couldnt_update_comment"));
        }

        let person_mention_id = read_person_mention.id;
        let read = data.read;
        let update_mention =
            move |conn: &'_ _| PersonMention::update_read(conn, person_mention_id, read);
        apply_label_write(
            blocking(context.pool(), update_mention)
                .await?
                .map_err(|e| LemmyError::from_error_message(e, "couldnt_update_comment"))?,
        );

        let person_mention_id = read_person_mention.id;
        let person_id = local_user_view.person.id;
        let person_mention_view = apply_label_read(
            blocking(context.pool(), move |conn| {
                PersonMentionView::read(conn, person_mention_id, Some(person_id))
            })
            .await??,
        );

        Ok(PersonMentionResponse {
            person_mention_view,
        })
    }
}
