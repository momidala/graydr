template "test_cache" {
  metadata {
    description = "Compile test for cache module."
    version     = "0.0.1"
  }

  parameters {
    cache {
      provider            = {}
      name                = {}
      region              = {}
      node_type           = {}
      resource_group_name = {}
    }
  }

  resource "main_cache" {
    module = "cache"
    inputs {
      name                = "$cache.name"
      region              = "$cache.region"
      node_type           = "$cache.node_type"
      resource_group_name = "$cache.resource_group_name"
    }
  }

  outputs {
    cache_endpoint = "${main_cache.endpoint}"
    cache_port     = "${main_cache.port}"
  }
}
