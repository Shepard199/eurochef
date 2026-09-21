import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
public class PiranhaHeightCallAssemblyEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address s=toAddr(0x00467A70L); disassemble(s);
  InstructionIterator it=currentProgram.getListing().getInstructions(s,true);
  int n=0;
  while(it.hasNext()&&n<120){
   Instruction ins=it.next();
   if(ins.getAddress().getOffset()>=0x00467B50L) break;
   println(ins.getAddress()+"  "+ins);
   for(Reference r:ins.getReferencesFrom()) if(r.getReferenceType().isCall()||r.getReferenceType().isJump()) println("  REF "+r.getReferenceType()+" -> "+r.getToAddress());
   n++;
  }
 }
}