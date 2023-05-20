use crate::Perform;
use actix_web::web::Data;
use crate::lemmy_api_common::{
  person::{BannedPersonsResponse, GetBannedPersons},
  utils::{blocking, get_local_user_view_from_jwt, is_admin, apply_label_read},
};
use crate::lemmy_db_views_actor::structs::PersonViewSafe;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;

#[async_trait::async_trait(?Send)]
impl Perform for GetBannedPersons {
  type Response = BannedPersonsResponse;

  #[cfg_attr(feature = "user-list-banned", dfpp::analyze)]
  async fn perform(
    &self,
    context: &Data<LemmyContext>,
    _websocket_id: Option<ConnectionId>,
  ) -> Result<Self::Response, LemmyError> {
    let data: &GetBannedPersons = self;
    let local_user_view =
      get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

    // Make sure user is an admin
    is_admin(&local_user_view)?;

    let banned = apply_label_read(blocking(context.pool(), PersonViewSafe::banned).await??);

    let res = Self::Response { banned };

    Ok(res)
  }
}
