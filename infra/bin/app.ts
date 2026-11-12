// AWS CDK app: describes the AWS resources for aws-burn-pile in TypeScript.
//
// How it works:
//   1. `bun run synth` runs this file. CDK turns the code into a CloudFormation
//      template (JSON) in cdk.out/. Nothing is created in AWS yet.
//   2. `bun run deploy` uploads that template plus the Docker image, and
//      CloudFormation creates or updates the real resources.
//   3. `bun run destroy` asks CloudFormation to delete everything in the stack.
//
// Ephemeral mode (`bun run deploy:eph`): deploys a separate stack that deletes
// itself after a TTL (default 60 minutes, change with `-- -c ttl=30`).

import * as path from "node:path";
import { App, CfnOutput, DefaultStackSynthesizer, Duration, RemovalPolicy, Stack } from "aws-cdk-lib";
import { Platform } from "aws-cdk-lib/aws-ecr-assets";
import * as iam from "aws-cdk-lib/aws-iam";
import * as lambda from "aws-cdk-lib/aws-lambda";
import * as logs from "aws-cdk-lib/aws-logs";
import * as scheduler from "aws-cdk-lib/aws-scheduler";

// The App is the root of the CDK program. It can hold one or more stacks.
const app = new App();

// Context values come from `-c key=value` on the command line (or cdk.json).
const ephemeral = app.node.tryGetContext("eph") === "true";
const ttlMinutes = Number(app.node.tryGetContext("ttl") ?? 60);

// A Stack is one CloudFormation stack: a group of resources that are deployed,
// updated, rolled back and deleted together. The name is what you see in the
// CloudFormation console. The ephemeral stack gets its own name, so it can
// live next to the normal one.
//
// `env` picks the target account and region. The CDK CLI fills in
// CDK_DEFAULT_ACCOUNT / CDK_DEFAULT_REGION from your --profile and AWS_REGION.
const stack = new Stack(app, ephemeral ? "AwsBurnPileEph" : "AwsBurnPile", {
  env: { account: process.env.CDK_DEFAULT_ACCOUNT, region: process.env.CDK_DEFAULT_REGION },
});

// Lambda = serverless compute. AWS runs the code only when a request arrives,
// and you pay per request and per millisecond. Nothing runs while idle.
//
// A Lambda can be a .zip of code or a container image. Here it's a container
// image built from ../Dockerfile, which holds the Rust/Leptos server plus the
// Lambda Web Adapter (it turns Lambda events into normal HTTP requests).
//
// The first argument (`stack`) is the parent, and "Web" is the construct ID.
// CDK uses the ID to build the resource's logical ID in the template.
const web = new lambda.DockerImageFunction(stack, "Web", {
  // On deploy, CDK runs `docker build` on the repo root and pushes the image to
  // the ECR (Elastic Container Registry) repository created by `cdk bootstrap`.
  code: lambda.DockerImageCode.fromImageAsset(path.join(import.meta.dirname, "..", ".."), {
    platform: Platform.LINUX_AMD64,
    // Files left out of the Docker build context (big or unrelated).
    exclude: ["target", "infra", ".git"],
  }),
  // CPU type. Must match the image platform above. ARM_64 (Graviton) is
  // cheaper, but the image would then have to be built for arm64.
  architecture: lambda.Architecture.X86_64,
  // Lambda gives CPU in proportion to memory. More memory = faster, but more expensive per ms.
  memorySize: 512,
  // Longest a single request may run before Lambda stops it (maximum is 15 min).
  timeout: Duration.seconds(30),
  // CloudWatch Logs gets everything the app prints. Old logs are deleted after
  // 7 days so storage costs stay low. By default CDK keeps (retains) the log
  // group even after `destroy`. The ephemeral stack deletes it too, so nothing is left behind.
  logGroup: new logs.LogGroup(stack, "WebLogs", {
    retention: logs.RetentionDays.ONE_WEEK,
    removalPolicy: ephemeral ? RemovalPolicy.DESTROY : RemovalPolicy.RETAIN,
  }),
});

// IAM (Identity and Access Management) controls who can do what.
//
// CDK automatically gives the Lambda an *execution role*: an IAM role the
// function assumes while it runs. AWS hands it short-lived credentials, so no
// access keys are stored anywhere. Lambda exposes them as environment
// variables, which the AWS SDK checks before the `profile` in burn-pile.toml.
//
// By default the role can only write logs. This statement adds the billing
// APIs the app calls (least privilege: only what's needed).
// Format of an action: "<service>:<ApiName>".
web.addToRolePolicy(
  new iam.PolicyStatement({
    actions: [
      "ce:GetCostAndUsage", // Cost Explorer: costs per month and service
      "budgets:ViewBudget", // AWS Budgets
      "invoicing:ListInvoiceSummaries", // invoices
      "freetier:GetFreeTierUsage", // free tier usage
    ],
    // These APIs don't support resource-level permissions, so "*" is required.
    resources: ["*"],
  }),
);

// Function URL = a free, built-in HTTPS endpoint for one Lambda
// (https://<id>.lambda-url.<region>.on.aws). It's a simpler alternative to
// API Gateway or a load balancer.
//
// NONE = public, no login required. AWS_IAM would require signed requests,
// which a browser can't make on its own.
const url = web.addFunctionUrl({ authType: lambda.FunctionUrlAuthType.NONE });

// Stack outputs are printed after `deploy` and shown in the CloudFormation
// console under "Outputs".
new CfnOutput(stack, "Url", { value: url.url });

if (ephemeral) {
  // Self-destruct timer. EventBridge Scheduler runs a one-time schedule at the
  // given UTC time. Its "universal target" can call almost any AWS API
  // directly; here it calls CloudFormation DeleteStack on this very stack.
  // The time is computed at synth time, so every deploy resets the timer.
  const expiresAt = new Date(Date.now() + ttlMinutes * 60_000).toISOString().slice(0, 19);

  // The scheduler needs its own IAM role (trusted by scheduler.amazonaws.com)
  // with permission to delete this stack.
  const role = new iam.Role(stack, "SelfDestructRole", {
    assumedBy: new iam.ServicePrincipal("scheduler.amazonaws.com"),
  });
  role.addToPolicy(
    new iam.PolicyStatement({
      actions: ["cloudformation:DeleteStack"],
      resources: [stack.formatArn({ service: "cloudformation", resource: "stack", resourceName: `${stack.stackName}/*` })],
    }),
  );
  // CloudFormation deletes the resources with the role CDK deployed the stack
  // with (created by `cdk bootstrap`), so the caller must be allowed to pass it.
  role.addToPolicy(
    new iam.PolicyStatement({
      actions: ["iam:PassRole"],
      resources: [
        `arn:aws:iam::${stack.account}:role/cdk-${DefaultStackSynthesizer.DEFAULT_QUALIFIER}-cfn-exec-role-${stack.account}-${stack.region}`,
      ],
    }),
  );

  // L1 construct (Cfn*) = a 1:1 mapping of the raw CloudFormation resource.
  new scheduler.CfnSchedule(stack, "SelfDestruct", {
    scheduleExpression: `at(${expiresAt})`,
    scheduleExpressionTimezone: "UTC",
    flexibleTimeWindow: { mode: "OFF" },
    target: {
      arn: "arn:aws:scheduler:::aws-sdk:cloudformation:deleteStack",
      roleArn: role.roleArn,
      input: JSON.stringify({ StackName: stack.stackName }),
    },
  });

  new CfnOutput(stack, "ExpiresAtUtc", { value: expiresAt });
}
