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
      vpc_id         = {}
      subnet_ids     = {}
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
      vpc_id         = "$db.vpc_id"
      subnet_ids     = "$db.subnet_ids"
    }
  }

  outputs {
    db_endpoint = "${main_db.endpoint}"
  }
}
