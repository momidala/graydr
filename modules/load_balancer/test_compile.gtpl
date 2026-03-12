template "test_load_balancer" {
  metadata {
    description = "Compile test for load_balancer module."
    version     = "0.0.1"
  }

  parameters {
    lb {
      provider            = {}
      name                = {}
      region              = {}
      resource_group_name = {}
    }
  }

  resource "main_lb" {
    module = "load_balancer"
    inputs {
      name                = "$lb.name"
      region              = "$lb.region"
      resource_group_name = "$lb.resource_group_name"
    }
  }

  outputs {
    lb_dns_name = "${main_lb.dns_name}"
  }
}
