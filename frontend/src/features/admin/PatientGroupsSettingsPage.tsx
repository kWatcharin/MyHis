import { patientGroupConfig } from './master/configs'
import { MasterSettingsPage } from './master/MasterSettingsPage'

export function PatientGroupsSettingsPage() {
  return <MasterSettingsPage config={patientGroupConfig} />
}
