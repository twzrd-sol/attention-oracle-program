# DigitalOcean Managed Redis (Terraform)

This module provisions a Managed Redis cluster on DigitalOcean and outputs a REDIS_URL suitable for BullMQ (ioredis). It uses TLS (rediss://) by default.

## Prerequisites

- Terraform >= 1.4
- `DO_TOKEN` environment variable with write access to your account
- Region and VPC that match your droplet (optional)

## Usage

```bash
cd infra/digitalocean/redis
export DO_TOKEN=your_token
terraform init
terraform apply -var="region=nyc3" -var="name=twzrd-redis" -auto-approve

# After apply, capture the REDIS_URL
terraform output -raw redis_url

# Set in .env
echo "REDIS_URL=$(terraform output -raw redis_url)" >> /home/twzrd/milo-token/.env

# Restart PM2 processes to pick up env
pm2 restart milo-aggregator tree-builder --update-env
```

## Notes

- Engine: Redis
- Version: 7
- Size: `db-s-1vcpu-1gb` (adjust as needed)
- If you prefer the control panel, create a Redis cluster there and copy the `rediss://` URI into `.env` as `REDIS_URL`.

