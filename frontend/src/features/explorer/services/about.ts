import { invoke } from '@tauri-apps/api/core'

export type AboutBuildInfo = {
  profile: string
  targetOs: string
  targetArch: string
  targetFamily: string
}

export type AboutInfo = {
  appName: string
  version: string
  changelog: string
  license: string
  thirdPartyNotices: string
  build: AboutBuildInfo
}

export const loadAboutInfo = () =>
  invoke<AboutInfo>('about_info')
