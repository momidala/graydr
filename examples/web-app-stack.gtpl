template "web-app-stack" {
  metadata {
    description = "Multi-cloud web application stack: network + database + storage + load balancer."
    version     = "1.0.0"
  }

  parameters {
    primary {
      provider = {}
      region   = {}
    }
    network_params {
      name       = {}
      cidr_block = {}
    }
    db_params {
      name           = {}
      db_username    = {}
      db_password    = {}
      instance_class = {}
    }
    storage_params {
      bucket_name = {}
    }
    lb_params {
      name                = {}
      resource_group_name = {}
    }
  }

  resource "net" {
    module = "network"
    inputs {
      name       = "$network_params.name"
      region     = "$primary.region"
      cidr_block = "$network_params.cidr_block"
    }
  }

  resource "db" {
    module = "relational_db"
    inputs {
      name           = "$db_params.name"
      region         = "$primary.region"
      db_username    = "$db_params.db_username"
      db_password    = "$db_params.db_password"
      instance_class = "$db_params.instance_class"
      vpc_id         = "${net.vpc_id}"
      subnet_ids     = "${net.subnet_ids}"
    }
  }

  resource "storage" {
    module = "object_storage"
    inputs {
      bucket_name = "$storage_params.bucket_name"
      region      = "$primary.region"
    }
  }

  resource "lb" {
    module = "load_balancer"
    inputs {
      name                = "$lb_params.name"
      region              = "$primary.region"
      resource_group_name = "$lb_params.resource_group_name"
    }
  }

  outputs {
    vpc_id      = "${net.vpc_id}"
    db_endpoint = "${db.endpoint}"
    bucket_url  = "${storage.bucket_url}"
    lb_dns_name = "${lb.dns_name}"
  }
}
