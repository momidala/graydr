template "bicep-cross-resource" {
  metadata {
    description = "Demonstrates cross-resource output wiring with Bicep-style output references"
    version     = "1.0.0"
  }

  parameters {
    primary {
      provider = {}
      region   = {}
    }
    infra {
      name = {}
    }
  }

  resource "primary_storage" {
    module = "storage"
    inputs {
      bucket_name = "$infra.name"
      region      = "$primary.region"
    }
  }

  resource "secondary_storage" {
    module = "storage"
    inputs {
      bucket_name = "derived-bucket"
      region      = "$primary.region"
      dependency_ref = "${primary_storage.bucket_url}"
    }
  }

  outputs {
    primary_blob_endpoint   = "${primary_storage.bucket_url}"
    secondary_blob_endpoint = "${secondary_storage.bucket_url}"
  }
}
