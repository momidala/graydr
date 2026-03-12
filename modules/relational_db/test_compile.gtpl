template "test_relational_db" {
  metadata {
    description = "Compile test for relational_db module."
    version     = "0.0.1"
  }

  parameters {
    db {
      provider       = {}
      name           = {}
      region         = {}
      db_username    = {}
      db_password    = {}
      instance_class = {}
    }
  }

  resource "main_db" {
    module = "relational_db"
    inputs {
      name           = "$db.name"
      region         = "$db.region"
      db_username    = "$db.db_username"
      db_password    = "$db.db_password"
      instance_class = "$db.instance_class"
    }
  }

  outputs {
    db_endpoint = "${main_db.endpoint}"
  }
}
