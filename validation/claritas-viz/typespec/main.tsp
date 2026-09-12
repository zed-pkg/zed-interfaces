import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace Zed.Claritas;

@id("ClaritasProvider")
enum ClaritasProvider { claritas: "claritas-viz" }
@id("VisualizationPurpose")
enum VisualizationPurpose { dependencyGraph: "dependency-graph" }
@id("ClaritasCapability")
enum ClaritasCapability { networkLayout: "network-layout" }
@id("VisualizationRuntime")
enum VisualizationRuntime { rust: "rust", typescript: "typescript", dart: "dart" }
@id("ClaritasVisualizationBinding")
model ClaritasVisualizationBinding {
  provider: ClaritasProvider;
  purposes: VisualizationPurpose[];
  capabilities: ClaritasCapability[];
  runtimes: VisualizationRuntime[];
  generatedWitnessOnly: boolean;
}
