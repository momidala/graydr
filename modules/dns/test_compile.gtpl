template "test_dns" {
  metadata {
    description = "Compile test for dns module."
    version     = "0.0.1"
  }

  parameters {
    dns {
      provider            = {}
      name                = {}
      region              = {}
      resource_group_name = {}
    }
  }

  resource "main_dns" {
    module = "dns"
    inputs {
      name                = "$dns.name"
      region              = "$dns.region"
      resource_group_name = "$dns.resource_group_name"
    }
  }

  outputs {
    dns_zone_id    = "${main_dns.zone_id}"
    dns_nameserver = "${main_dns.nameservers}"
  }
}
