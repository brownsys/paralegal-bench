#[cfg(feature = "full")]
use crate::lemmy_db_schema::schema::secret;

#[derive(Clone)]
#[cfg_attr(feature = "full", derive(Queryable, Identifiable))]
#[cfg_attr(feature = "full", table_name = "secret")]
pub struct Secret {
  pub id: i32,
  pub jwt_secret: String,
}
