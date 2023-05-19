use crate::lemmy_api_crud::PerformCrud;
use actix_web::web::Data;
use crate::lemmy_api_common::{
  person::{CreatePrivateMessage, PrivateMessageResponse},
  utils::{
    blocking,
    check_person_block,
    get_local_user_view_from_jwt,
    get_user_lang,
    send_email_to_user,
    apply_label_read,
    apply_label_write
  },
};
use crate::lemmy_apub::{
  generate_local_apub_endpoint,
  protocol::activities::{
    create_or_update::private_message::CreateOrUpdatePrivateMessage,
    CreateOrUpdateType,
  },
  EndpointType,
};
use crate::lemmy_db_schema::{
  source::private_message::{PrivateMessage, PrivateMessageForm},
  traits::Crud,
};
use crate::lemmy_db_views::structs::LocalUserView;
use crate::lemmy_utils::{error::LemmyError, utils::remove_slurs, ConnectionId};
use crate::lemmy_websocket::{send::send_pm_ws_message, LemmyContext, UserOperationCrud};

#[async_trait::async_trait(?Send)]
impl PerformCrud for CreatePrivateMessage {
  type Response = PrivateMessageResponse;

  #[tracing::instrument(skip(self, context, websocket_id))]
  #[cfg_attr(feature = "private-message-create", dfpp::analyze)]
  async fn perform(
    &self,
    context: &Data<LemmyContext>,
    websocket_id: Option<ConnectionId>,
  ) -> Result<PrivateMessageResponse, LemmyError> {
    let data: &CreatePrivateMessage = self;
    let local_user_view =
      get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

    let content_slurs_removed =
      remove_slurs(&data.content.to_owned(), &context.settings().slur_regex());

    apply_label_read(check_person_block(local_user_view.person.id, data.recipient_id, context.pool()).await?);

    let private_message_form = PrivateMessageForm {
      content: content_slurs_removed.to_owned(),
      creator_id: local_user_view.person.id,
      recipient_id: data.recipient_id,
      ..PrivateMessageForm::default()
    };

    let inserted_private_message = match apply_label_write(blocking(context.pool(), move |conn| {
      PrivateMessage::create(conn, &private_message_form)
    })
    .await?)
    {
      Ok(private_message) => private_message,
      Err(e) => {
        return Err(LemmyError::from_error_message(
          e,
          "couldnt_create_private_message",
        ));
      }
    };

    let inserted_private_message_id = inserted_private_message.id;
    let protocol_and_hostname = context.settings().get_protocol_and_hostname();
    let updated_private_message = apply_label_write(blocking(
      context.pool(),
      move |conn| -> Result<PrivateMessage, LemmyError> {
        let apub_id = generate_local_apub_endpoint(
          EndpointType::PrivateMessage,
          &inserted_private_message_id.to_string(),
          &protocol_and_hostname,
        )?;
        Ok(PrivateMessage::update_ap_id(
          conn,
          inserted_private_message_id,
          apub_id,
        )?)
      },
    )
    .await?)
    .map_err(|e| e.with_message("couldnt_create_private_message"))?;

    CreateOrUpdatePrivateMessage::send(
      updated_private_message.into(),
      &local_user_view.person.into(),
      CreateOrUpdateType::Create,
      context,
    )
    .await?;

    let res = send_pm_ws_message(
      inserted_private_message.id,
      UserOperationCrud::CreatePrivateMessage,
      websocket_id,
      context,
    )
    .await?;

    // Send email to the local recipient, if one exists
    if res.private_message_view.recipient.local {
      let recipient_id = data.recipient_id;
      let local_recipient = apply_label_read(blocking(context.pool(), move |conn| {
        LocalUserView::read_person(conn, recipient_id)
      })
      .await??);
      let lang = get_user_lang(&local_recipient);
      let inbox_link = format!("{}/inbox", context.settings().get_protocol_and_hostname());
      send_email_to_user(
        &local_recipient,
        &lang.notification_private_message_subject(&local_recipient.person.name),
        &lang.notification_private_message_body(
          &inbox_link,
          &content_slurs_removed,
          &local_recipient.person.name,
        ),
        context.settings(),
      );
    }

    Ok(res)
  }
}
