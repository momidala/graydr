template "test_secret_manager" {
  metadata {
    description = "Compile test for secret_manager module."
    version     = "0.0.1"
  }

  parameters {
    secret_manager {
      provider            = {}
      name                = {}
      region              = {}
      resource_group_name = {}
      tenant_id           = {}
    }
  }

  resource "main_secret_manager" {
    module = "secret_manager"
    inputs {
      name                = "$secret_manager.name"
      region              = "$secret_manager.region"
      resource_group_name = "$secret_manager.resource_group_name"
      tenant_id           = "$secret_manager.tenant_id"
    }
  }

  outputs {
    secret_arn = "${main_secret_manager.secret_arn}"
    secret_id  = "${main_secret_manager.secret_id}"
  }
}
