use crate::lemmy_api_crud::PerformCrud;
use actix_web::web::Data;
use crate::lemmy_api_common::{
  comment::{CommentResponse, GetComment},
  utils::{blocking, check_private_instance, get_local_user_view_from_jwt_opt},
};
use crate::lemmy_db_views::structs::CommentView;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;

#[async_trait::async_trait(?Send)]
impl PerformCrud for GetComment {
  type Response = CommentResponse;

  #[tracing::instrument(skip(context, _websocket_id))]
  #[cfg_attr(feature = "comment-read", dfpp::analyze)]
  async fn perform(
    &self,
    context: &Data<LemmyContext>,
    _websocket_id: Option<ConnectionId>,
  ) -> Result<Self::Response, LemmyError> {
    let data = self;
    let local_user_view =
      get_local_user_view_from_jwt_opt(data.auth.as_ref(), context.pool(), context.secret())
        .await?;

    check_private_instance(&local_user_view, context.pool()).await?;

    let person_id = local_user_view.map(|u| u.person.id);
    let id = data.id;
    let comment_view = blocking(context.pool(), move |conn| {
      CommentView::read(conn, id, person_id)
    })
    .await?
    .map_err(|e| LemmyError::from_error_message(e, "couldnt_find_comment"))?;

    Ok(Self::Response {
      comment_view,
      form_id: None,
      recipient_ids: Vec::new(),
    })
  }
}
