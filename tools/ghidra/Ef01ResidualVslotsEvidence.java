import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class Ef01ResidualVslotsEvidence extends GhidraScript {
 private void dc(long a,String n,DecompInterface d)throws Exception{Function f=getFunctionContaining(toAddr(a));println(String.format("\n===%s %08X %s===",n,a,f==null?"<missing>":f.getName()));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}}
 @Override public void run() throws Exception{DecompInterface d=new DecompInterface();d.openProgram(currentProgram);dc(0x00463920L,"EF01_A4",d);dc(0x00453480L,"BASE_A4",d);dc(0x00463B80L,"EF01_170",d);dc(0x00451F40L,"BASE_170",d);d.dispose();}
}