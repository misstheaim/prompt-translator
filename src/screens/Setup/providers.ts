import type { ProviderStatus } from "../../bindings/ProviderStatus";

/** Which providers are ready to translate right now. */
export function workingProviders(status: ProviderStatus | null): {
  ollama: boolean;
  claudeCode: boolean;
} {
  return {
    ollama: status?.ollama.reachable === true && status.ollama.selectedModelInstalled,
    claudeCode: status?.claude?.found === true && status.claude.signedIn !== false,
  };
}
