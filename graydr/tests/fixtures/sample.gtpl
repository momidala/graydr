template "data-platform" {
  metadata {
    description = "Minimal data platform template"
    version     = "0.1.0"
  }

  parameters {
    primary_db {
      provider = {}
      region   = {}
      size     = {}
    }
  }

  resource "main_storage" {
    module = "storage"
    inputs {
      bucket_name = "$primary_db.name"
      region      = "$primary_db.region"
    }
  }

  outputs {
    storage_url = "${main_storage.bucket_url}"
  }
}
