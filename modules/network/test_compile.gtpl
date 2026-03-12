template "test_network" {
  metadata {
    description = "Compile test for network module."
    version     = "0.0.1"
  }

  parameters {
    net {
      provider   = {}
      name       = {}
      region     = {}
      cidr_block = {}
    }
  }

  resource "main_network" {
    module = "network"
    inputs {
      name       = "$net.name"
      region     = "$net.region"
      cidr_block = "$net.cidr_block"
    }
  }

  outputs {
    vpc_id = "${main_network.vpc_id}"
  }
}
