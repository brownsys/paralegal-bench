use crate::lemmy_api_crud::PerformCrud;
use actix_web::web::Data;
use crate::lemmy_api_common::{
  person::{DeletePrivateMessage, PrivateMessageResponse},
  utils::{blocking, get_local_user_view_from_jwt, apply_label_read, apply_label_write},
};
use crate::lemmy_apub::activities::deletion::send_apub_delete_private_message;
use crate::lemmy_db_schema::{source::private_message::PrivateMessage, traits::Crud};
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::{send::send_pm_ws_message, LemmyContext, UserOperationCrud};

#[async_trait::async_trait(?Send)]
impl PerformCrud for DeletePrivateMessage {
  type Response = PrivateMessageResponse;

  #[tracing::instrument(skip(self, context, websocket_id))]
  #[cfg_attr(feature = "private-message-delete", dfpp::analyze)]
  async fn perform(
    &self,
    context: &Data<LemmyContext>,
    websocket_id: Option<ConnectionId>,
  ) -> Result<PrivateMessageResponse, LemmyError> {
    let data: &DeletePrivateMessage = self;
    let local_user_view =
      get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

    // Checking permissions
    let private_message_id = data.private_message_id;
    let orig_private_message = apply_label_read(blocking(context.pool(), move |conn| {
      PrivateMessage::read(conn, private_message_id)
    })
    .await??);
    if local_user_view.person.id != orig_private_message.creator_id {
      return Err(LemmyError::from_message("no_private_message_edit_allowed"));
    }

    // Doing the update
    let private_message_id = data.private_message_id;
    let deleted = data.deleted;
    let updated_private_message = apply_label_write(blocking(context.pool(), move |conn| {
      PrivateMessage::update_deleted(conn, private_message_id, deleted)
    })
    .await?)
    .map_err(|e| LemmyError::from_error_message(e, "couldnt_update_private_message"))?;

    // Send the apub update
    send_apub_delete_private_message(
      &local_user_view.person.into(),
      updated_private_message,
      data.deleted,
      context,
    )
    .await?;

    let op = UserOperationCrud::DeletePrivateMessage;
    send_pm_ws_message(data.private_message_id, op, websocket_id, context).await
  }
}
