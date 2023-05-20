use crate::lemmy_db_schema::newtypes::LocalUserId;

#[cfg(feature = "full")]
use crate::lemmy_db_schema::schema::email_verification;

#[derive(Clone)]
#[cfg_attr(feature = "full", derive(Queryable, Identifiable))]
#[cfg_attr(feature = "full", table_name = "email_verification")]
pub struct EmailVerification {
  pub id: i32,
  pub local_user_id: LocalUserId,
  pub email: String,
  pub verification_code: String,
  pub published: chrono::NaiveDateTime,
}

#[cfg_attr(feature = "full", derive(Insertable, AsChangeset))]
#[cfg_attr(feature = "full", table_name = "email_verification")]
pub struct EmailVerificationForm {
  pub local_user_id: LocalUserId,
  pub email: String,
  pub verification_token: String,
}
