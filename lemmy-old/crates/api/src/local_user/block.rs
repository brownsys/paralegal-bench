use crate::Perform;
use actix_web::web::Data;
use crate::lemmy_api_common::{
  person::{BlockPerson, BlockPersonResponse},
  utils::{blocking, get_local_user_view_from_jwt, apply_label_read, apply_label_write},
};
use crate::lemmy_db_schema::{
  source::person_block::{PersonBlock, PersonBlockForm},
  traits::Blockable,
};
use crate::lemmy_db_views_actor::structs::PersonViewSafe;
use crate::lemmy_utils::{error::LemmyError, ConnectionId};
use crate::lemmy_websocket::LemmyContext;

#[async_trait::async_trait(?Send)]
impl Perform for BlockPerson {
  type Response = BlockPersonResponse;

  #[tracing::instrument(skip(context, _websocket_id))]
  #[cfg_attr(feature = "user-block", dfpp::analyze)]
  async fn perform(
    &self,
    context: &Data<LemmyContext>,
    _websocket_id: Option<ConnectionId>,
  ) -> Result<BlockPersonResponse, LemmyError> {
    let data: &BlockPerson = self;
    let local_user_view =
      get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

    let target_id = data.person_id;
    let person_id = local_user_view.person.id;

    // Don't let a person block themselves
    if target_id == person_id {
      return Err(LemmyError::from_message("cant_block_yourself"));
    }

    let person_block_form = PersonBlockForm {
      person_id,
      target_id,
    };

    let target_person_view = apply_label_read(blocking(context.pool(), move |conn| {
      PersonViewSafe::read(conn, target_id)
    })
    .await??);

    if target_person_view.person.admin {
      return Err(LemmyError::from_message("cant_block_admin"));
    }

    if data.block {
      let block = move |conn: &'_ _| PersonBlock::block(conn, &person_block_form);
      apply_label_write(blocking(context.pool(), block)
        .await?
        .map_err(|e| LemmyError::from_error_message(e, "person_block_already_exists"))?);
    } else {
      let unblock = move |conn: &'_ _| PersonBlock::unblock(conn, &person_block_form);
      apply_label_write(blocking(context.pool(), unblock)
        .await?
        .map_err(|e| LemmyError::from_error_message(e, "person_block_already_exists"))?);
    }

    let res = BlockPersonResponse {
      person_view: target_person_view,
      blocked: data.block,
    };

    Ok(res)
  }
}
