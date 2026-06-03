export function isH264StreamEndpoint(endpoint: string) {
  return /^https?:\/\/[^/]+\/h264(?:\?|$)/i.test(endpoint);
}

export function endpointForControlHost(endpoint: string, controlAddress: string) {
  const endpointUrl = new URL(endpoint);
  const controlUrl = new URL(controlAddress);
  endpointUrl.hostname = controlUrl.hostname;
  return endpointUrl.toString();
}
