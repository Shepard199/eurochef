import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class MessageBoxCreateCallbacksEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void one(long x)throws Exception{Address a=toAddr(x);disassemble(a);Function f=getFunctionAt(a);if(f==null){try{f=createFunction(a,null);}catch(Exception e){println("CREATE_FAIL "+Long.toHexString(x)+" "+e.getMessage());}}println(String.format("\n=== 0x%08X ===",x));if(f==null){println("NO_FUNCTION");return;}println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());else println("ERR="+r.getErrorMessage());}
 public void run()throws Exception{d.openProgram(currentProgram);long[] a={0x004a7cb0L,0x004a7d80L,0x004a7e00L,0x0049c200L,0x004a6700L};for(long x:a)one(x);d.dispose();}
}