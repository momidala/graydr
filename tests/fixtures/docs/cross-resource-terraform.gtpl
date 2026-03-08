template "terraform-cross-resource" {
  metadata {
    description = "Demonstrates cross-resource output wiring for Terraform"
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
      bucket_name = "logs-bucket"
      region      = "$primary.region"
      dependency_ref = "${primary_storage.bucket_url}"
    }
  }

  outputs {
    primary_bucket_url   = "${primary_storage.bucket_url}"
    secondary_bucket_url = "${secondary_storage.bucket_url}"
  }
}
