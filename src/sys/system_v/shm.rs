//! Safe wrapper around a SystemV shared memory segment
//!

use std::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr::{null, null_mut},
};

use crate::Result;
use crate::{errno::Errno, sys::stat::Mode};

use libc::{self, c_int, c_void, key_t, shmid_ds};

#[derive(Debug, Clone)]
/// Safe wrapper around a SystemV shared memory segment
///
/// The shared memory segment size is equal to the size of T.
///
/// This is a smart pointer, and so implement the [`Deref`] and [`DerefMut`] traits.
/// This means that you can work with the shared memory zone like you would with a [`Box`].
///
/// This type does not automatically create or destroy a shared memory segment,
/// but only attach and detach from them using RAII.
///
/// To create one, use [`SharedMemory::shmget`], with the key [`ShmgetFlag::IPC_CREAT`].\
/// To delete one, use [`SharedMemory::shmctl`], with the key [`ShmctlFlag::IPC_RMID`].
///
/// # Example
///
/// ```no_run
/// # use nix::errno::Errno;
/// # use nix::sys::system_v::shm::*;
/// # use nix::sys::stat::Mode;
/// #
/// struct MyData(i64);
/// const MY_KEY: i32 = 1337;
///
/// let id = SharedMemory::<MyData>::shmget(
///     MY_KEY,
///     ShmgetFlag::IPC_CREAT | ShmgetFlag::IPC_EXCL,
///     Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO,
/// )?;
/// let mut shared_memory = SharedMemory::<MyData>::new(
///     id,
///     None,
///     ShmatFlag::empty(),
///     Mode::empty(),
/// )?;
///
/// // This is writing to the stored [`MyData`] struct
/// shared_memory.0 = 0xDEADBEEF;
/// # Ok::<(), Errno>(())
/// ```
///
pub struct SharedMemory<T> {
    id: i32,
    shm: ManuallyDrop<Box<T>>,
}

impl<T> Deref for SharedMemory<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.shm
    }
}
impl<T> DerefMut for SharedMemory<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.shm
    }
}

impl<T> Drop for SharedMemory<T> {
    fn drop(&mut self) {
        Self::shmdt(self).expect("SharedMemory detach from SysV IPC");
    }
}

impl<T> SharedMemory<T> {
    /// Create a new SharedMemory object
    ///
    /// Attach to an existing SystemV shared memory segment.
    ///
    /// To create a new segment, use [`SharedMemory::shmget`], with the key [`ShmgetFlag::IPC_CREAT`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use nix::errno::Errno;
    /// # use nix::sys::system_v::shm::*;
    /// # use nix::sys::stat::Mode;
    /// #
    /// struct MyData(i64);
    /// const MY_KEY: i32 = 1337;
    ///
    /// let mut shared_memory = SharedMemory::<MyData>::new(
    ///     id,
    ///     None,
    ///     ShmatFlag::empty(),
    ///     Mode::empty(),
    /// )?;
    /// # Ok::<(), Errno>(())
    /// ```
    ///
    pub fn new(
        shmid: c_int,
        shmaddr: Option<c_void>,
        shmat_flag: ShmatFlag,
        mode: Mode,
    ) -> Result<Self> {
        unsafe {
            Ok(Self {
                id: shmid,
                shm: ManuallyDrop::new(Box::from_raw(Self::shmat(
                    shmid, shmaddr, shmat_flag, mode,
                )?)),
            })
        }
    }

    /// Creates and returns a new, or returns an existing, System V shared memory
    /// segment identifier.
    ///
    /// For more information, see [`shmget(2)`].
    ///
    /// # Example
    ///
    /// ## Creating a shared memory zone
    /// 
    /// ```no_run
    /// # use nix::errno::Errno;
    /// # use nix::sys::system_v::shm::*;
    /// # use nix::sys::stat::Mode;
    /// #
    /// struct MyData(i64);
    /// const MY_KEY: i32 = 1337;
    ///
    /// let id = SharedMemory::<MyData>::shmget(
    ///     MY_KEY,
    ///     ShmgetFlag::IPC_CREAT | ShmgetFlag::IPC_EXCL,
    ///     Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO,
    /// )?;
    /// # Ok::<(), Errno>(())
    /// ```
    ///
    /// [`shmget(2)`]: https://man7.org/linux/man-pages/man2/shmget.2.html
    ///
    pub fn shmget(
        key: key_t,
        shmget_flag: ShmgetFlag,
        mode: Mode,
    ) -> Result<i32> {
        let size = std::mem::size_of::<T>();
        let flags = mode.bits() as i32 | shmget_flag.bits();
        Errno::result(unsafe { libc::shmget(key, size, flags) })
    }

    /// Performs control operation specified by `cmd` on the System V shared
    /// memory segment given by `shmid`.
    ///
    /// For more information, see [`shmctl(2)`].
    ///
    /// # Example
    ///
    /// ## Deleting a shared memory zone
    /// 
    /// ```no_run
    /// # use nix::errno::Errno;
    /// # use nix::sys::system_v::shm::*;
    /// # use nix::sys::stat::Mode;
    /// #
    /// struct MyData(i64);
    /// const ID: i32 = 1337;
    ///
    /// let mut shared_memory = SharedMemory::<MyData>::new(
    ///     ID,
    ///     None,
    ///     ShmatFlag::empty(),
    ///     Mode::empty(),
    /// )?;
    ///
    /// shared_memory.shmctl(ShmctlFlag::IPC_RMID, None, Mode::empty()) = 0xDEADBEEF;
    /// # Ok::<(), Errno>(())
    /// ```
    ///
    /// [`shmctl(2)`]: https://man7.org/linux/man-pages/man2/shmctl.2.html
    pub fn shmctl(
        &self,
        shmctl_flag: ShmctlFlag,
        buf: Option<shmid_ds>,
        mode: Mode,
    ) -> Result<c_int> {
        let buf_ptr: *mut shmid_ds = match buf {
            Some(mut ptr) => &mut ptr,
            None => null_mut(),
        };
        let flags = mode.bits() as i32 | shmctl_flag.bits();
        Errno::result(unsafe { libc::shmctl(self.id, flags, buf_ptr) })
    }

    // -- Private --

    /// Attaches the System V shared memory segment identified by `shmid` to the
    /// address space of the calling process.
    ///
    /// This is called automatically on [`SharedMemory::new`].
    /// 
    /// For more information, see [`shmat(2)`].
    ///
    /// [`shmat(2)`]: https://man7.org/linux/man-pages/man2/shmat.2.html
    fn shmat(
        shmid: c_int,
        shmaddr: Option<c_void>,
        shmat_flag: ShmatFlag,
        mode: Mode,
    ) -> Result<*mut T> {
        let shmaddr_ptr: *const c_void = match shmaddr {
            Some(mut ptr) => &mut ptr,
            None => null(),
        };
        let flags = mode.bits() as i32 | shmat_flag.bits();
        Errno::result(unsafe { libc::shmat(shmid, shmaddr_ptr, flags) })
            .map(|ok| ok.cast::<T>())
    }

    /// Performs the reverse of [`SharedMemory::shmat`], detaching the shared memory segment at
    /// the given address from the address space of the calling process.
    ///
    /// This is called automatically on [`Drop`].
    /// 
    /// For more information, see [`shmdt(2)`].
    ///
    /// [`shmdt(2)`]: https://man7.org/linux/man-pages/man2/shmdt.2.html
    fn shmdt(&self) -> Result<()> {
        let shmaddr_ref: *const T = &**self;
        Errno::result(unsafe { libc::shmdt(shmaddr_ref.cast::<c_void>()) })
            .map(drop)
    }
}

libc_bitflags!(
    /// Valid flags for the third parameter of the function [`shmget`]
    pub struct ShmgetFlag: c_int
    {
        /// A new shared memory segment is created if key has this value.
        IPC_PRIVATE;
        /// Create a new segment.
        /// If this flag is not used, then shmget() will find the segment
        /// associated with key and check to see if the user has permission
        /// to access the segment.
        IPC_CREAT;
        /// This flag is used with IPC_CREAT to ensure that this call creates
        /// the segment.  If the segment already exists, the call fails.
        IPC_EXCL;
        /// Allocate the segment using "huge" pages.  See the Linux kernel
        /// source file Documentation/admin-guide/mm/hugetlbpage.rst for
        /// further information.
        #[cfg(linux)]
        SHM_HUGETLB;
        // TODO: Does not exist in libc/linux, but should? Maybe open an issue in their repo
        // SHM_HUGE_2MB;
        // TODO: Same for this one
        // SHM_HUGE_1GB;
        /// This flag serves the same purpose as the mmap(2) MAP_NORESERVE flag.
        /// Do not reserve swap space for this segment. When swap space is
        /// reserved, one has the guarantee that it is possible to modify the
        /// segment. When swap space is not reserved one might get SIGSEGV upon
        /// a write if no physical memory is available. See also the discussion
        /// of the file /proc/sys/vm/overcommit_memory in proc(5).
        #[cfg(linux)]
        SHM_NORESERVE;
    }
);
libc_bitflags! {
    /// Valid flags for the third parameter of the function [`shmat`]
    pub struct ShmatFlag: c_int
    {
        /// Allow the contents of the segment to be executed. The caller must
        /// have execute permission on the segment.
        #[cfg(linux)]
        SHM_EXEC;
        /// This flag specifies that the mapping of the segment should replace
        /// any existing mapping in the range starting at shmaddr and
        /// continuing for the size of the segment.
        /// (Normally, an EINVAL error would result if a mapping already exists
        /// in this address range.)
        /// In this case, shmaddr must not be NULL.
        #[cfg(linux)]
        SHM_REMAP;
        /// Attach the segment for read-only access. The process must have read
        /// permission for the segment. If this flag is not specified, the
        /// segment is attached for read and write access, and the process must
        /// have read and write permission for the segment.
        /// There is no notion of a write-only shared memory segment.
        SHM_RDONLY;
        /// TODO: I have no clue at what this does
        SHM_RND;
    }
}

libc_bitflags!(
    /// Valid flags for the second parameter of the function [`shmctl`]
    pub struct ShmctlFlag: c_int {
        /// Returns the index of the highest used entry in the kernel's internal
        /// array recording information about all shared memory segment
        #[cfg(linux)]
        IPC_INFO;
        /// Write the values of some members of the shmid_ds structure pointed
        /// to by buf to the kernel data structure associated with this shared
        /// memory segment, updating also its shm_ctime member.
        ///
        /// The following fields are updated: shm_perm.uid,
        /// shm_perm.gid, and (the least significant 9 bits of)
        /// shm_perm.mode.
        ///
        /// The effective UID of the calling process must match the owner
        /// (shm_perm.uid) or creator (shm_perm.cuid) of the shared memory
        /// segment, or the caller must be privileged.
        IPC_SET;
        /// Copy information from the kernel data structure associated with
        /// shmid into the shmid_ds structure pointed to by buf.
        /// The caller must have read permission on the shared memory segment.
        IPC_STAT;
        /// Mark the segment to be destroyed. The segment will actually be
        /// destroyed only after the last process detaches it
        /// (i.e., when the shm_nattch member of the associated structure
        /// shmid_ds is zero).
        /// The caller must be the owner or creator of the segment,
        /// or be privileged. The buf argument is ignored.
        ///
        /// If a segment has been marked for destruction, then the
        /// (nonstandard) SHM_DEST flag of the shm_perm.mode field in the
        /// associated data structure retrieved by IPC_STAT will be set.
        ///
        /// The caller must ensure that a segment is eventually destroyed;
        /// otherwise its pages that were faulted in will remain in memory
        /// or swap.
        ///
        /// See also the description of /proc/sys/kernel/shm_rmid_forced
        /// in proc(5).
        IPC_RMID;
        // not available in libc/linux, but should be?
        // SHM_INFO;
        // SHM_STAT;
        // SHM_STAT_ANY;
        /// Prevent swapping of the shared memory segment. The caller must
        /// fault in any pages that are required to be present after locking is
        /// enabled.
        /// If a segment has been locked, then the (nonstandard) SHM_LOCKED
        /// flag of the shm_perm.mode field in the associated data structure
        /// retrieved by IPC_STAT will be set.
        #[cfg(linux)]
        SHM_LOCK;
        /// Unlock the segment, allowing it to be swapped out.
        #[cfg(linux)]
        SHM_UNLOCK;
    }
);

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    static SHM_MTX: Mutex<()> = Mutex::new(());

    const SHM_TEST: i32 = 1337;

    #[derive(Debug)]
    /// Test struct used to store some data on the shared memory zone
    ///
    struct TestData {
        data: i64,
    }

    #[derive(Debug)]
    struct FixtureShm {
        ipc: SharedMemory<TestData>,
    }

    impl FixtureShm {
        fn setup() -> Result<Self> {
            let id = SharedMemory::<TestData>::shmget(
                SHM_TEST,
                ShmgetFlag::IPC_CREAT | ShmgetFlag::IPC_EXCL,
                Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO,
            )?;
            Ok(Self {
                ipc: SharedMemory::<TestData>::new(
                    id,
                    None,
                    ShmatFlag::empty(),
                    Mode::empty(),
                )?,
            })
        }
    }

    impl Drop for FixtureShm {
        fn drop(&mut self) {
            let _ = self
                .ipc
                .shmctl(ShmctlFlag::IPC_RMID, None, Mode::empty())
                .map_err(|_| {
                    panic!("Failed to delete the test shared memory zone")
                });
        }
    }

    #[test]
    fn create_ipc() -> Result<()> {
        let _m = SHM_MTX.lock();

        FixtureShm::setup()?;
        Ok(())
    }

    #[test]
    fn create_ipc_already_exist() -> Result<()> {
        let _m = SHM_MTX.lock();

        // Keep the IPC in scope, so we don't destroy it
        let _ipc = FixtureShm::setup()?;
        let expected = Errno::EEXIST;
        let actual = FixtureShm::setup().expect_err("Return EExist");

        assert_eq!(expected, actual);
        Ok(())
    }

    #[test]
    fn create_ipc_and_get_value() -> Result<()> {
        let _m = SHM_MTX.lock();

        let mut sem = FixtureShm::setup()?;
        let expected = 0xDEADBEEF;
        sem.ipc.data = expected;
        assert_eq!(expected, sem.ipc.data);
        Ok(())
    }
}
