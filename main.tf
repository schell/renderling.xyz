provider "aws" {
  region = "us-west-1"
}

provider "aws" {
  alias  = "oregon"
  region = "us-west-2"
}

provider "aws" {
  alias  = "virginia"
  region = "us-east-1"
}

terraform {
  backend "s3" {
    bucket = "renderling-terraform"
    key    = "renderling"
    region = "us-east-1"
  }
}

module "production" {
  #source = "git::https://github.com/schell/mars.git//modules/static_site"
  source      = "../mars/modules/static_site"
  zone_id     = "Z053462417S8CIXH18CJ5"
  domain_name = "renderling.xyz"
}

module "staging" {
  #source = "git::https://github.com/schell/mars.git//modules/static_site"
  source      = "../mars/modules/static_site"
  zone_id     = "Z053462417S8CIXH18CJ5"
  domain_name = "staging.renderling.xyz"
}
