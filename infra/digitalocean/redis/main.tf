terraform {
  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = "~> 2.36"
    }
  }
}

provider "digitalocean" {
  token = var.do_token
}

variable "do_token" {
  type        = string
  description = "DigitalOcean API token"
  default     = null
}

variable "name" {
  type        = string
  description = "Cluster name"
  default     = "twzrd-redis"
}

variable "region" {
  type        = string
  description = "Region (e.g., nyc3)"
  default     = "nyc3"
}

variable "size" {
  type        = string
  description = "Node size slug"
  default     = "db-s-1vcpu-1gb"
}

resource "digitalocean_database_cluster" "redis" {
  name       = var.name
  engine     = "redis"
  version    = "7"
  region     = var.region
  size       = var.size
  node_count = 1
}

output "redis_host" {
  value = digitalocean_database_cluster.redis.host
}

output "redis_port" {
  value = digitalocean_database_cluster.redis.port
}

output "redis_password" {
  value     = digitalocean_database_cluster.redis.password
  sensitive = true
}

output "redis_url" {
  value     = "rediss://${digitalocean_database_cluster.redis.user}:${digitalocean_database_cluster.redis.password}@${digitalocean_database_cluster.redis.host}:${digitalocean_database_cluster.redis.port}"
  sensitive = true
}

