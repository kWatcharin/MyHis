import { nationalityConfig } from './master/configs'
import { MasterSettingsPage } from './master/MasterSettingsPage'

export function NationalitiesSettingsPage() {
  return <MasterSettingsPage config={nationalityConfig} />
}
