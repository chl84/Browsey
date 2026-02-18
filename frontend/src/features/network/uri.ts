import { classifyNetworkUri, type NetworkUriClassification } from './services'

const NOT_URI: NetworkUriClassification = {
  kind: 'not_uri',
  scheme: null,
  normalizedUri: null,
}

const classifySafe = async (value: string): Promise<NetworkUriClassification> => {
  try {
    return await classifyNetworkUri(value.trim())
  } catch {
    return NOT_URI
  }
}

export const uriScheme = async (value: string): Promise<string | null> =>
  (await classifySafe(value)).scheme

export const isMountUri = async (value: string): Promise<boolean> =>
  (await classifySafe(value)).kind !== 'not_uri'

export const isKnownNetworkUriScheme = async (scheme: string | null): Promise<boolean> => {
  if (!scheme) return false
  const classification = await classifySafe(`${scheme}://example`)
  return classification.kind === 'mountable' || classification.kind === 'external'
}

export const isMountableUri = async (value: string): Promise<boolean> =>
  (await classifySafe(value)).kind === 'mountable'

export const isExternallyOpenableUri = async (value: string): Promise<boolean> =>
  (await classifySafe(value)).kind === 'external'
