//! The external conformance suites spit's report projects its cases onto, in report order.

mod cluster_validation;
mod hck_persistent_reservation;
mod hck_scsi_assertions;
mod hlk_fuzz;
mod hlk_nvme_scsi_compliance;
mod hlk_scsi_compliance;
mod hlk_storage;
mod msdsm;
mod unh_iscsi_chap;
mod unh_iscsi_error_recovery;
mod unh_iscsi_full_feature;
mod unh_iscsi_interop;
mod unh_iscsi_login;
mod unh_iscsi_multiconnection;
mod unh_nvme_command_set;
mod unh_nvme_interop;
mod unh_nvme_tcp;
mod unh_sas_application;
mod unh_sas_error_handling;
mod unh_sas_link;
mod unh_sas_transport;
mod vmware_vaai;
mod whcp_storage;

use super::suite::Suite;

pub const SUITES: [&Suite; 23] = [
    &hlk_scsi_compliance::HLK_SCSI_COMPLIANCE,
    &hck_scsi_assertions::HCK_SCSI_ASSERTIONS,
    &hck_persistent_reservation::HCK_PERSISTENT_RESERVATION,
    &hlk_nvme_scsi_compliance::HLK_NVME_SCSI_COMPLIANCE,
    &hlk_fuzz::HLK_FUZZ,
    &hlk_storage::HLK_STORAGE,
    &whcp_storage::WHCP_STORAGE,
    &cluster_validation::CLUSTER_VALIDATION,
    &msdsm::MSDSM,
    &vmware_vaai::VMWARE_VAAI,
    &unh_iscsi_login::UNH_ISCSI_LOGIN,
    &unh_iscsi_chap::UNH_ISCSI_CHAP,
    &unh_iscsi_full_feature::UNH_ISCSI_FULL_FEATURE,
    &unh_iscsi_error_recovery::UNH_ISCSI_ERROR_RECOVERY,
    &unh_iscsi_multiconnection::UNH_ISCSI_MULTICONNECTION,
    &unh_iscsi_interop::UNH_ISCSI_INTEROP,
    &unh_nvme_command_set::UNH_NVME_COMMAND_SET,
    &unh_nvme_tcp::UNH_NVME_TCP,
    &unh_nvme_interop::UNH_NVME_INTEROP,
    &unh_sas_application::UNH_SAS_APPLICATION,
    &unh_sas_transport::UNH_SAS_TRANSPORT,
    &unh_sas_error_handling::UNH_SAS_ERROR_HANDLING,
    &unh_sas_link::UNH_SAS_LINK,
];
