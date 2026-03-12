template "test_container_registry" {
  metadata {
    description = "Compile test for container_registry module."
    version     = "0.0.1"
  }

  parameters {
    reg {
      provider            = {}
      name                = {}
      region              = {}
      resource_group_name = {}
    }
  }

  resource "main_registry" {
    module = "container_registry"
    inputs {
      name                = "$reg.name"
      region              = "$reg.region"
      resource_group_name = "$reg.resource_group_name"
    }
  }

  outputs {
    registry_url = "${main_registry.registry_url}"
  }
}
