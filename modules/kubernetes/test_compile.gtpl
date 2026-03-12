template "test_kubernetes" {
  metadata {
    description = "Compile test for kubernetes module."
    version     = "0.0.1"
  }

  parameters {
    k8s {
      provider            = {}
      name                = {}
      region              = {}
      vpc_id              = {}
      subnet_ids          = {}
      instance_class      = {}
      resource_group_name = {}
    }
  }

  resource "main_cluster" {
    module = "kubernetes"
    inputs {
      name                = "$k8s.name"
      region              = "$k8s.region"
      vpc_id              = "$k8s.vpc_id"
      subnet_ids          = "$k8s.subnet_ids"
      instance_class      = "$k8s.instance_class"
      resource_group_name = "$k8s.resource_group_name"
    }
  }

  outputs {
    cluster_endpoint   = "${main_cluster.cluster_endpoint}"
    kubeconfig_command = "${main_cluster.kubeconfig_command}"
  }
}
