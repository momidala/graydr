template "test_object_storage" {
  metadata {
    description = "Compile test for object_storage module."
    version     = "0.0.1"
  }
  parameters {
    storage {
      provider    = {}
      bucket_name = {}
      region      = {}
    }
  }
  resource "main_storage" {
    module = "object_storage"
    inputs {
      bucket_name = "$storage.bucket_name"
      region      = "$storage.region"
    }
  }
  outputs {
    bucket_url = "${main_storage.bucket_url}"
  }
}
