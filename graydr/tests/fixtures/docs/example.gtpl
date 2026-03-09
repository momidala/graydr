template "web-application" {
  metadata {
    description = "Three-tier web application template with storage and database"
    version     = "1.0.0"
  }

  parameters {
    primary {
      provider = {}
      region   = {}
      env      = {}
    }
    app {
      name     = {}
      db_pass  = {}
    }
  }

  resource "app_storage" {
    module = "storage"
    inputs {
      bucket_name = "$app.name"
      region      = "$primary.region"
    }
  }

  resource "app_database" {
    module = "database"
    inputs {
      db_name     = "$app.name"
      db_password = "$app.db_pass"
      region      = "$primary.region"
      dependency_ref = "${app_storage.bucket_url}"
    }
  }

  outputs {
    storage_url       = "${app_storage.bucket_url}"
    db_endpoint       = "${app_database.endpoint}"
    db_connection_str = "${app_database.connection_string}"
  }
}
